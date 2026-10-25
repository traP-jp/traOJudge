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
        "/api/v1/users/user_123",
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["code"], "unexpected_error");
    assert!(UserId::new(body["requestId"].as_str().unwrap()).is_ok());
    assert_eq!(body.as_object().unwrap().len(), 3);
    assert!(!body.to_string().contains("secret"));
}

#[tokio::test]
async fn invalid_id_is_404_without_querying_repository() {
    for id in ["bad.id", &"a".repeat(129)] {
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
    // Re-running migrations must leave existing databases usable.
    MIGRATOR.run(&pool).await.unwrap();
    sqlx::query("INSERT INTO users (id, display_name, revision) VALUES ($1, $2, $3)")
        .bind("user_123")
        .bind("利用者 🦀")
        .bind("revision_1")
        .execute(&pool)
        .await
        .unwrap();

    let router = traojudge_backend_app::build_router(PgUserProfileRepository::new(pool.clone()));
    let (status, body) = get(router.clone(), "/api/v1/users/user_123").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "id": "user_123", "displayName": "利用者 🦀", "bio": "",
            "githubId": "", "atcoderId": "", "xId": "", "revision": "revision_1"
        })
    );

    sqlx::query(
        "UPDATE users SET bio = $1, github_id = $2, atcoder_id = $3, x_id = $4,
         revision = $5 WHERE id = $6",
    )
    .bind("自己紹介")
    .bind("github-name")
    .bind("atcoder_name")
    .bind("x_name")
    .bind("revision_2")
    .bind("user_123")
    .execute(&pool)
    .await
    .unwrap();
    let (status, body) = get(router.clone(), "/api/v1/users/user_123").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "id": "user_123", "displayName": "利用者 🦀", "bio": "自己紹介",
            "githubId": "github-name", "atcoderId": "atcoder_name", "xId": "x_name",
            "revision": "revision_2"
        })
    );

    let (status, body) = get(router, "/api/v1/users/missing").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["code"], "not_found");
    assert!(UserId::new(body["requestId"].as_str().unwrap()).is_ok());

    // Database constraints must protect records even outside the application.
    for (id, name, revision) in [
        ("user_123", "duplicate", "v1"),
        ("bad.id", "valid", "v1"),
        ("empty_name", "", "v1"),
        ("long_name", &"あ".repeat(81), "v1"),
        ("empty_revision", "valid", ""),
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
