use async_trait::async_trait;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use traojudge_core::{
    domain::model::user::{PublicUserProfile, UserId},
    ports::repository::user::UserProfileRepository,
};
use traojudge_infra::postgres::{MIGRATOR, users::PgUserProfileRepository};
use uuid::Uuid;

const USER_ID: &str = "a8098c1a-f86e-4f70-917b-72f7c091480c";
const MISSING_ID: &str = "e9c3d4ab-27e5-43fc-9289-b5d81c327027";

struct FailingRepository;

#[async_trait]
impl UserProfileRepository for FailingRepository {
    type Error = std::io::Error;

    async fn find_public_profile(
        &self,
        _: &UserId,
    ) -> Result<Option<PublicUserProfile>, Self::Error> {
        Err(std::io::Error::other("secret database connection details"))
    }
}

async fn get(router: Router, path: &str) -> (StatusCode, Value) {
    let response = router
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    assert_eq!(response.headers()["content-type"], "application/json");
    let body = to_bytes(response.into_body(), 16 * 1024).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap())
}

#[tokio::test]
async fn repository_failure_is_500_without_leaking_internal_details() {
    let (status, body) = get(
        traojudge_backend_app::build_router(FailingRepository),
        &format!("/api/v1/users/{USER_ID}"),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["code"], "unexpected_error");
    assert!(Uuid::parse_str(body["requestId"].as_str().unwrap()).is_ok());
    assert_eq!(body.as_object().unwrap().len(), 3);
    assert!(!body.to_string().contains("secret"));
}

#[tokio::test]
async fn invalid_id_is_404_without_querying_repository() {
    for id in ["user_123", "bad.id", &"a".repeat(129)] {
        let (status, body) = get(
            traojudge_backend_app::build_router(FailingRepository),
            &format!("/api/v1/users/{id}"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "not_found");
    }
}

// SQLx creates an isolated database and drops it after the test.
#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL pointing to PostgreSQL with CREATE DATABASE permission"]
async fn public_profile_from_postgres(pool: PgPool) {
    let user_id = Uuid::parse_str(USER_ID).unwrap();
    // Re-running migrations must leave existing databases usable.
    MIGRATOR.run(&pool).await.unwrap();
    sqlx::query("INSERT INTO users (id, display_name, revision) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind("利用者 🦀")
        .bind("revision_1")
        .execute(&pool)
        .await
        .unwrap();

    let router = traojudge_backend_app::build_router(PgUserProfileRepository::new(pool.clone()));
    let (status, body) = get(router.clone(), &format!("/api/v1/users/{USER_ID}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "id": USER_ID, "displayName": "利用者 🦀", "bio": "",
            "githubId": "", "atcoderId": "", "xId": "", "revision": "revision_1"
        })
    );

    let (status, canonical_body) = get(
        router.clone(),
        &format!("/api/v1/users/{}", USER_ID.to_ascii_uppercase()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(canonical_body, body);

    sqlx::query(
        "UPDATE users SET bio = $1, github_id = $2, atcoder_id = $3, x_id = $4,
         revision = $5 WHERE id = $6",
    )
    .bind("自己紹介")
    .bind("github-name")
    .bind("atcoder_name")
    .bind("x_name")
    .bind("revision_2")
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();
    let (status, body) = get(router.clone(), &format!("/api/v1/users/{USER_ID}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "id": USER_ID, "displayName": "利用者 🦀", "bio": "自己紹介",
            "githubId": "github-name", "atcoderId": "atcoder_name", "xId": "x_name",
            "revision": "revision_2"
        })
    );

    let (status, body) = get(router, &format!("/api/v1/users/{MISSING_ID}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], "not_found");
    assert!(Uuid::parse_str(body["requestId"].as_str().unwrap()).is_ok());

    // Database constraints must protect records even outside the application.
    assert!(
        sqlx::query(
            "INSERT INTO users (id, display_name, revision) VALUES ('not-a-uuid', 'valid', 'v1')",
        )
        .execute(&pool)
        .await
        .is_err()
    );
    for (id, name, revision) in [
        (user_id, "duplicate", "v1"),
        (Uuid::new_v4(), "", "v1"),
        (Uuid::new_v4(), &"あ".repeat(81), "v1"),
        (Uuid::new_v4(), "valid", ""),
    ] {
        assert!(
            sqlx::query("INSERT INTO users (id, display_name, revision) VALUES ($1, $2, $3)")
                .bind(id)
                .bind(name)
                .bind(revision)
                .execute(&pool)
                .await
                .is_err()
        );
    }
}

#[sqlx::test(migrations = false)]
#[ignore = "requires DATABASE_URL pointing to PostgreSQL with CREATE DATABASE permission"]
async fn uuid_migration_preserves_existing_profile(pool: PgPool) {
    sqlx::raw_sql(include_str!(
        "../../traojudge-infra/src/postgres/migrations/20261025000000_create_users.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO users (id, display_name, revision) VALUES ($1, $2, $3)")
        .bind(USER_ID)
        .bind("既存利用者")
        .bind("revision_1")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::raw_sql(include_str!(
        "../../traojudge-infra/src/postgres/migrations/20261025000001_use_uuid_user_ids.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();

    let (status, body) = get(
        traojudge_backend_app::build_router(PgUserProfileRepository::new(pool)),
        &format!("/api/v1/users/{USER_ID}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"], USER_ID);
    assert_eq!(body["displayName"], "既存利用者");
}
