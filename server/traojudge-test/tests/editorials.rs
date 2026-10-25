use axum::{
    Router,
    body::{Body, to_bytes},
    http::{HeaderMap, Request, StatusCode},
};
use chrono::Duration;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;
use traojudge_backend_app::http::{build_router, middleware::origin::WebOrigin};
use traojudge_core::{
    application::{
        account::AccountService, editorial::EditorialService, session::SessionService,
        users::GetUserProfile,
    },
    domain::model::{
        account::{AuthenticatedSession, Secret},
        editorial::{EditorialChanges, EditorialCreationKey, EditorialError, NewEditorial},
        user::UserId,
    },
    ports::repository::editorial::{EditorialUnitOfWork, EditorialUnitOfWorkProvider},
};
use traojudge_infra::{
    account_services::UnconfiguredAccountServices,
    postgres::{
        accounts::PgAccountUnitOfWorkProvider, editorial::PgEditorialUnitOfWorkProvider,
        sessions::PgSessionRepository, users::PgUserProfileRepository,
    },
};
use uuid::Uuid;

const ORIGIN: &str = "https://judge.example";
struct Problems(PgPool);
impl Problems {
    fn new(pool: &PgPool) -> Self {
        Self(pool.clone())
    }
    async fn public(&self) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO problems (id,visibility) VALUES ($1,'public')")
            .bind(id)
            .execute(&self.0)
            .await
            .unwrap();
        id
    }
    async fn private(&self, owner: UserId) -> Uuid {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO problems (id) VALUES ($1)")
            .bind(id)
            .execute(&self.0)
            .await
            .unwrap();
        sqlx::query("INSERT INTO account_roles (user_id,role,scope_kind,scope_id) VALUES ($1,'problem-owner','problem',$2)")
            .bind(owner.as_uuid()).bind(id).execute(&self.0).await.unwrap();
        id
    }
}
struct Actor {
    id: Uuid,
    session: AuthenticatedSession,
    secret: String,
}
async fn actor(pool: &PgPool, role: Option<&str>) -> Actor {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id,display_name,revision) VALUES ($1,'利用者','initial')")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
    if let Some(role) = role {
        sqlx::query("INSERT INTO account_roles (user_id,role,scope_kind) VALUES ($1,$2,'service')")
            .bind(id)
            .bind(role)
            .execute(pool)
            .await
            .unwrap();
    }
    let secret = Secret::random();
    let session_id = Uuid::new_v4();
    sqlx::query("INSERT INTO web_sessions (id,user_id,token_digest,expires_at) VALUES ($1,$2,$3,clock_timestamp()+INTERVAL '1 day')")
        .bind(session_id).bind(id).bind(secret.digest()).execute(pool).await.unwrap();
    Actor {
        id,
        session: AuthenticatedSession {
            id: session_id,
            user_id: UserId::new(id),
        },
        secret: secret.expose().to_owned(),
    }
}
fn app(pool: &PgPool) -> Router {
    build_router(
        GetUserProfile::new(PgUserProfileRepository::new(pool.clone())),
        AccountService::new(
            PgAccountUnitOfWorkProvider::new(pool.clone()),
            UnconfiguredAccountServices,
        ),
        SessionService::new(PgSessionRepository::new(pool.clone())),
        EditorialService::new(
            PgEditorialUnitOfWorkProvider::new(pool.clone()),
            Duration::days(1),
        )
        .unwrap(),
        WebOrigin::parse(ORIGIN).unwrap(),
    )
}
struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: Value,
}
struct Client {
    router: Router,
    secret: Option<String>,
}
impl Client {
    fn new(router: &Router, actor: Option<&Actor>) -> Self {
        Self {
            router: router.clone(),
            secret: actor.map(|actor| actor.secret.clone()),
        }
    }
    async fn call(
        &self,
        verb: &str,
        path: &str,
        headers: &[(&str, &str)],
        body: Option<Value>,
    ) -> Reply {
        let mut builder = Request::builder().method(verb).uri(path);
        if let Some(secret) = &self.secret {
            builder = builder.header("cookie", format!("traojudge_session={secret}"));
        }
        if !matches!(verb, "GET" | "HEAD") && !headers.iter().any(|(name, _)| *name == "origin") {
            builder = builder.header("origin", ORIGIN);
        }
        for (name, value) in headers {
            builder = builder.header(*name, *value);
        }
        let body = match body {
            Some(body) => {
                builder = builder.header("content-type", "application/json");
                Body::from(body.to_string())
            }
            None => Body::empty(),
        };
        let response = self
            .router
            .clone()
            .oneshot(builder.body(body).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        assert_eq!(headers["cache-control"], "no-store");
        let bytes = to_bytes(response.into_body(), 2 * 1024 * 1024)
            .await
            .unwrap();
        let body = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        Reply {
            status,
            headers,
            body,
        }
    }
    async fn create(&self, problem_id: Uuid, key: &str) -> Reply {
        self.call(
            "POST",
            "/api/v1/editorials",
            &[("idempotency-key", key)],
            Some(json!({"problemId":problem_id,"title":"解説 🦀","body":"本文"})),
        )
        .await
    }
}
fn path(reply: &Reply) -> String {
    format!("/api/v1/editorials/{}", reply.body["id"].as_str().unwrap())
}
async fn count(pool: &PgPool, table: &str) -> i64 {
    let query = match table {
        "editorials" => "SELECT count(*) FROM editorials",
        "editorial_creation_receipts" => "SELECT count(*) FROM editorial_creation_receipts",
        _ => panic!("unsupported test table"),
    };
    sqlx::query_scalar(query).fetch_one(pool).await.unwrap()
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn creation_is_private_atomic_and_idempotent_even_after_editing(pool: PgPool) {
    let own = actor(&pool, Some("standard-user")).await;
    let problems = Problems::new(&pool);
    let problem = problems.public().await;
    let router = app(&pool);
    let client = Client::new(&router, Some(&own));
    let anonymous = Client::new(&router, None);
    let before = client.call("GET", "/api/v1/me", &[], None).await;
    let (first, retry) = tokio::join!(
        client.create(problem, "create-1"),
        client.create(problem, "create-1")
    );
    assert_eq!(first.status, StatusCode::CREATED);
    assert_eq!(retry.status, StatusCode::CREATED);
    assert_eq!(first.body, retry.body);
    assert_eq!(first.headers["etag"], retry.headers["etag"]);
    assert_eq!(first.body["visibility"], "private");
    assert_eq!(first.body["authorId"], own.id.to_string());
    assert_eq!(count(&pool, "editorials").await, 1);
    assert_eq!(count(&pool, "editorial_creation_receipts").await, 1);
    let after = client.call("GET", "/api/v1/me", &[], None).await;
    assert_ne!(before.headers["etag"], after.headers["etag"]);
    assert!(
        after.body["roles"]
            .as_array()
            .unwrap()
            .iter()
            .any(|role| role["role"] == "editiorial-author"
                && role["scope"]["editorialId"] == first.body["id"])
    );
    let resource = path(&first);
    assert_eq!(
        anonymous.call("GET", &resource, &[], None).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        client.call("GET", &resource, &[], None).await.body,
        first.body
    );
    let updated = client
        .call(
            "PATCH",
            &resource,
            &[("if-match", first.headers["etag"].to_str().unwrap())],
            Some(json!({"title":"改訂"})),
        )
        .await;
    assert_eq!(updated.status, StatusCode::OK);
    assert_eq!(updated.body["body"], "本文");
    let replay = client.create(problem, "create-1").await;
    assert_eq!(replay.body, first.body);
    assert_eq!(replay.headers["etag"], first.headers["etag"]);
    let conflict = client
        .call(
            "POST",
            "/api/v1/editorials",
            &[("idempotency-key", "create-1")],
            Some(json!({"problemId":problem,"title":"別の題名","body":"本文"})),
        )
        .await;
    assert_eq!(conflict.status, StatusCode::CONFLICT);
    let second_user = actor(&pool, Some("standard-user")).await;
    let independent = Client::new(&router, Some(&second_user))
        .create(problem, "create-1")
        .await;
    assert_eq!(independent.status, StatusCode::CREATED);
    assert_ne!(independent.body["id"], first.body["id"]);
    sqlx::query("UPDATE editorial_creation_receipts SET expires_at = clock_timestamp() - INTERVAL '1 second' WHERE user_id = $1").bind(own.id).execute(&pool).await.unwrap();
    let expired = client.create(problem, "create-1").await;
    assert_eq!(expired.status, StatusCode::CREATED);
    assert_ne!(expired.body["id"], first.body["id"]);
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn mutation_inputs_origin_and_roles_are_enforced(pool: PgPool) {
    let own = actor(&pool, Some("standard-user")).await;
    let other = actor(&pool, Some("writer")).await;
    let problems = Problems::new(&pool);
    let problem = problems.public().await;
    let router = app(&pool);
    let client = Client::new(&router, Some(&own));
    assert_eq!(
        Client::new(&router, None)
            .create(problem, "new")
            .await
            .status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        Client::new(&router, Some(&other))
            .create(problem, "new")
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    for headers in [
        vec![],
        vec![("idempotency-key", "bad key")],
        vec![("idempotency-key", "a"), ("idempotency-key", "b")],
    ] {
        assert_eq!(
            client
                .call(
                    "POST",
                    "/api/v1/editorials",
                    &headers,
                    Some(json!({"problemId":problem,"title":"a","body":"b"}))
                )
                .await
                .status,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(
        client
            .call(
                "POST",
                "/api/v1/editorials",
                &[
                    ("idempotency-key", "a"),
                    ("origin", "https://untrusted.example")
                ],
                Some(json!({}))
            )
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    for body in [
        json!({"problemId":problem,"title":"","body":"ok"}),
        json!({"problemId":problem,"title":"あ".repeat(201),"body":"ok"}),
        json!({"problemId":problem,"title":"ok","body":"あ".repeat(200001)}),
        json!({"problemId":problem,"title":"ok","body":null}),
        json!({"problemId":"not-uuid","title":"ok","body":"ok"}),
        json!({"problemId":problem,"title":"ok","body":"ok","authorId":own.id}),
        json!({"problemId":problem,"title":"ok","body":"\0"}),
    ] {
        assert_eq!(
            client
                .call(
                    "POST",
                    "/api/v1/editorials",
                    &[("idempotency-key", "invalid")],
                    Some(body)
                )
                .await
                .status,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    assert_eq!(count(&pool, "editorials").await, 0);
    let created = client
        .call(
            "POST",
            "/api/v1/editorials",
            &[("idempotency-key", "unicode")],
            Some(json!({"problemId":problem,"title":"あ".repeat(200),"body":"🦀".repeat(200000)})),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED);
    let resource = path(&created);
    let tag = created.headers["etag"].to_str().unwrap();
    assert_eq!(
        client
            .call("PATCH", &resource, &[], Some(json!({"title":"updated"})))
            .await
            .status,
        StatusCode::PRECONDITION_REQUIRED
    );
    assert_eq!(
        client
            .call(
                "PATCH",
                &resource,
                &[("if-match", "*")],
                Some(json!({"title":"updated"}))
            )
            .await
            .status,
        StatusCode::BAD_REQUEST
    );
    for body in [
        json!({}),
        json!({"title":null}),
        json!({"body":12}),
        json!({"problemId":Uuid::new_v4()}),
        json!({"visibility":"public"}),
    ] {
        assert_eq!(
            client
                .call("PATCH", &resource, &[("if-match", tag)], Some(body))
                .await
                .status,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    assert_eq!(
        client
            .call("GET", "/api/v1/editorials/not-a-uuid", &[], None)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn concurrent_edits_preserve_one_revision_and_parent_and_author(pool: PgPool) {
    let own = actor(&pool, Some("standard-user")).await;
    let problems = Problems::new(&pool);
    let problem = problems.public().await;
    let router = app(&pool);
    let client = Client::new(&router, Some(&own));
    let created = client.create(problem, "new").await;
    let resource = path(&created);
    let tag = created.headers["etag"].to_str().unwrap();
    let headers = [("if-match", tag)];
    let (a, b) = tokio::join!(
        client.call("PATCH", &resource, &headers, Some(json!({"title":"first"}))),
        client.call(
            "PATCH",
            &resource,
            &headers,
            Some(json!({"title":"second"}))
        )
    );
    assert_eq!(
        [a.status, b.status]
            .iter()
            .filter(|&&status| status == StatusCode::OK)
            .count(),
        1
    );
    assert_eq!(
        [a.status, b.status]
            .iter()
            .filter(|&&status| status == StatusCode::PRECONDITION_FAILED)
            .count(),
        1
    );
    let current = client.call("GET", &resource, &[], None).await;
    assert_eq!(current.body["problemId"], problem.to_string());
    assert_eq!(current.body["authorId"], own.id.to_string());
    assert_eq!(current.body["visibility"], "private");
    assert_eq!(current.body["createdAt"], created.body["createdAt"]);
    assert_ne!(current.headers["etag"], created.headers["etag"]);
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn public_pages_exclude_private_rows_and_bind_cursors_to_the_problem(pool: PgPool) {
    let own = actor(&pool, Some("standard-user")).await;
    let problems = Problems::new(&pool);
    let problem = problems.public().await;
    let another = problems.public().await;
    let restricted = problems.private(own.session.user_id).await;
    let router = app(&pool);
    let client = Client::new(&router, Some(&own));
    let anonymous = Client::new(&router, None);
    let mut visible = Vec::new();
    for index in 0..4 {
        let created = client.create(problem, &format!("e-{index}")).await;
        visible.push(
            created.body["id"]
                .as_str()
                .unwrap()
                .parse::<Uuid>()
                .unwrap(),
        );
    }
    client.create(problem, "private").await;
    let unrelated = client.create(another, "unrelated").await;
    let hidden_parent = client.create(restricted, "restricted").await;
    visible.push(unrelated.body["id"].as_str().unwrap().parse().unwrap());
    visible.push(hidden_parent.body["id"].as_str().unwrap().parse().unwrap());
    // Fixture-only publication; the undecided publication HTTP operations stay disabled.
    sqlx::query("UPDATE editorials SET visibility = 'public',created_at = '2026-01-01T00:00:00Z' WHERE id = ANY($1)").bind(&visible).execute(&pool).await.unwrap();
    let mut cursor = None;
    let mut seen = Vec::new();
    loop {
        let query = cursor
            .as_ref()
            .map(|value| format!("&cursor={value}"))
            .unwrap_or_default();
        let response = anonymous
            .call(
                "GET",
                &format!("/api/v1/problems/{problem}/editorials?limit=2{query}"),
                &[],
                None,
            )
            .await;
        assert_eq!(response.status, StatusCode::OK);
        let items = response.body["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        seen.extend(
            items
                .iter()
                .map(|item| item["id"].as_str().unwrap().parse::<Uuid>().unwrap()),
        );
        if response.body["nextCursor"].is_null() {
            break;
        }
        let next = response.body["nextCursor"].as_str().unwrap().to_owned();
        assert_eq!(
            anonymous
                .call(
                    "GET",
                    &format!("/api/v1/problems/{another}/editorials?cursor={next}"),
                    &[],
                    None
                )
                .await
                .status,
            StatusCode::UNPROCESSABLE_ENTITY
        );
        cursor = Some(next);
    }
    visible.truncate(4);
    visible.sort_unstable();
    assert_eq!(seen, visible);
    assert_eq!(
        anonymous
            .call("GET", &path(&unrelated), &[], None)
            .await
            .status,
        StatusCode::OK
    );
    assert_eq!(
        anonymous
            .call("GET", &path(&hidden_parent), &[], None)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        anonymous
            .call(
                "GET",
                &format!("/api/v1/problems/{restricted}/editorials"),
                &[],
                None
            )
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    for query in ["limit=0", "limit=101", "cursor=invalid"] {
        assert_eq!(
            anonymous
                .call(
                    "GET",
                    &format!("/api/v1/problems/{problem}/editorials?{query}"),
                    &[],
                    None
                )
                .await
                .status,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    assert_eq!(
        anonymous
            .call(
                "GET",
                &format!("/api/v1/problems/{}/editorials", Uuid::new_v4()),
                &[],
                None
            )
            .await
            .status,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn author_management_is_scoped_idempotent_and_does_not_imply_publication_or_delete(
    pool: PgPool,
) {
    let owner = actor(&pool, Some("standard-user")).await;
    let target = actor(&pool, None).await;
    let admin = actor(&pool, Some("service-admin")).await;
    let problems = Problems::new(&pool);
    let problem = problems.public().await;
    let router = app(&pool);
    let own_client = Client::new(&router, Some(&owner));
    let target_client = Client::new(&router, Some(&target));
    let admin_client = Client::new(&router, Some(&admin));
    let created = own_client.create(problem, "one").await;
    let other = own_client.create(problem, "two").await;
    let resource = path(&created);
    let authors = format!("{resource}/authors/{}", target.id);
    assert_eq!(
        target_client.call("GET", &resource, &[], None).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        own_client.call("PUT", &authors, &[], None).await.status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        admin_client.call("GET", &resource, &[], None).await.status,
        StatusCode::NOT_FOUND
    );
    let before = target_client.call("GET", "/api/v1/me", &[], None).await;
    let first = admin_client.call("PUT", &authors, &[], None).await;
    assert_eq!(first.status, StatusCode::OK);
    assert_eq!(first.body["changed"], true);
    assert_eq!(
        first.body["assignment"]["scope"]["editorialId"],
        created.body["id"]
    );
    let after = target_client.call("GET", "/api/v1/me", &[], None).await;
    assert_ne!(before.headers["etag"], after.headers["etag"]);
    let retry = admin_client.call("PUT", &authors, &[], None).await;
    assert_eq!(retry.body["changed"], false);
    assert_eq!(retry.body["assignment"], first.body["assignment"]);
    assert_eq!(
        target_client
            .call("GET", "/api/v1/me", &[], None)
            .await
            .headers["etag"],
        after.headers["etag"]
    );
    assert_eq!(
        target_client
            .call("GET", &path(&other), &[], None)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    let updated = target_client
        .call(
            "PATCH",
            &resource,
            &[("if-match", created.headers["etag"].to_str().unwrap())],
            Some(json!({"body":"担当者による修正"})),
        )
        .await;
    assert_eq!(updated.status, StatusCode::OK);
    assert_eq!(updated.body["authorId"], owner.id.to_string());
    for client in [&target_client] {
        for (verb, suffix) in [
            ("DELETE", ""),
            ("PUT", "/publication"),
            ("DELETE", "/publication"),
        ] {
            let response = client
                .call(
                    verb,
                    &format!("{resource}{suffix}"),
                    &[("if-match", updated.headers["etag"].to_str().unwrap())],
                    None,
                )
                .await;
            assert_eq!(response.status, StatusCode::FORBIDDEN);
            assert_eq!(response.body["code"], "forbidden");
        }
    }
    let missing = format!("{resource}/authors/{}", Uuid::new_v4());
    assert_eq!(
        admin_client.call("PUT", &missing, &[], None).await.status,
        StatusCode::NOT_FOUND
    );
    for _ in 0..2 {
        assert_eq!(
            admin_client
                .call("DELETE", &authors, &[], None)
                .await
                .status,
            StatusCode::NO_CONTENT
        );
    }
    assert_eq!(
        target_client.call("GET", &resource, &[], None).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        own_client.call("GET", &resource, &[], None).await.status,
        StatusCode::OK
    );
    let persisted = own_client.call("GET", &resource, &[], None).await;
    assert_eq!(persisted.body["visibility"], "private");
    assert_eq!(count(&pool, "editorials").await, 2);
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn creation_failure_rolls_back_editorial_author_and_account_revision(pool: PgPool) {
    let own = actor(&pool, Some("standard-user")).await;
    let problems = Problems::new(&pool);
    let problem = problems.public().await;
    let router = app(&pool);
    let client = Client::new(&router, Some(&own));
    let before = client.call("GET", "/api/v1/me", &[], None).await;
    // Fail after the editorial and author role were inserted to test the whole UoW.
    sqlx::raw_sql("CREATE FUNCTION reject_editorial_receipt() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'private persistence failure'; END; $$; CREATE TRIGGER reject_editorial_receipt BEFORE INSERT ON editorial_creation_receipts FOR EACH ROW EXECUTE FUNCTION reject_editorial_receipt();").execute(&pool).await.unwrap();
    let response = client.create(problem, "new").await;
    assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(response.body["code"], "unexpected_error");
    assert!(
        !response
            .body
            .to_string()
            .contains("private persistence failure")
    );
    assert_eq!(count(&pool, "editorials").await, 0);
    assert_eq!(count(&pool, "editorial_creation_receipts").await, 0);
    let after = client.call("GET", "/api/v1/me", &[], None).await;
    assert_eq!(after.body, before.body);
    assert_eq!(after.headers["etag"], before.headers["etag"]);
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn missing_problems_are_not_accepted_and_sessions_are_rechecked(pool: PgPool) {
    let own = actor(&pool, Some("standard-user")).await;
    let problems = Problems::new(&pool);
    let problem = problems.public().await;
    let router = app(&pool);
    let client = Client::new(&router, Some(&own));
    assert_eq!(
        client.create(Uuid::new_v4(), "missing").await.status,
        StatusCode::NOT_FOUND
    );
    let created = client.create(problem, "new").await;
    let provider = PgEditorialUnitOfWorkProvider::new(pool.clone());
    for rollback in [true, false] {
        let mut tx = provider.begin(Some(own.session), &[]).await.unwrap();
        let mut editorial = tx
            .find(created.body["id"].as_str().unwrap().parse().unwrap())
            .await
            .unwrap()
            .unwrap();
        editorial.title = "uncommitted".to_owned();
        tx.save(&editorial).await.unwrap();
        if rollback {
            tx.rollback().await.unwrap();
        } else {
            drop(tx);
        }
        assert_eq!(
            client.call("GET", &path(&created), &[], None).await.body["title"],
            "解説 🦀"
        );
    }
    sqlx::query("UPDATE web_sessions SET revoked_at = clock_timestamp() WHERE id = $1")
        .bind(own.session.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        provider.begin(Some(own.session), &[]).await,
        Err(EditorialError::Unauthenticated)
    ));
    assert_eq!(
        client.create(problem, "revoked").await.status,
        StatusCode::UNAUTHORIZED
    );
}

#[test]
fn editorial_validation_uses_character_lengths_and_unambiguous_input_digests() {
    let problem = Uuid::new_v4();
    assert!(
        NewEditorial {
            problem_id: problem,
            title: "あ".repeat(200),
            body: "🦀".repeat(200000)
        }
        .validate()
        .is_ok()
    );
    assert!(EditorialChanges::default().validate().is_err());
    let a = NewEditorial {
        problem_id: problem,
        title: "ab".to_owned(),
        body: "c".to_owned(),
    };
    let b = NewEditorial {
        problem_id: problem,
        title: "a".to_owned(),
        body: "bc".to_owned(),
    };
    assert_ne!(a.digest(), b.digest());
    for key in ["", "bad key", "あ", &"a".repeat(129)] {
        assert!(EditorialCreationKey::parse(key).is_err());
    }
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn original_author_and_admin_can_publish_unpublish_and_delete(pool: PgPool) {
    let own = actor(&pool, Some("standard-user")).await;
    let admin = actor(&pool, Some("service-admin")).await;
    let other = actor(&pool, Some("standard-user")).await;
    let problems = Problems::new(&pool);
    let problem = problems.public().await;
    let router = app(&pool);
    let owner = Client::new(&router, Some(&own));
    let administrator = Client::new(&router, Some(&admin));
    let outsider = Client::new(&router, Some(&other));
    let anonymous = Client::new(&router, None);
    let created = owner.create(problem, "lifecycle").await;
    let resource = path(&created);
    let publication = format!("{resource}/publication");
    assert_eq!(
        anonymous.call("DELETE", &resource, &[], None).await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        outsider.call("DELETE", &resource, &[], None).await.status,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        owner.call("PUT", &publication, &[], None).await.status,
        StatusCode::PRECONDITION_REQUIRED
    );
    let initial_tag = created.headers["etag"].to_str().unwrap();
    assert_eq!(
        outsider
            .call("PUT", &publication, &[("if-match", initial_tag)], None)
            .await
            .status,
        StatusCode::FORBIDDEN
    );
    let published = owner
        .call("PUT", &publication, &[("if-match", initial_tag)], None)
        .await;
    assert_eq!(published.status, StatusCode::OK);
    assert_eq!(published.body["visibility"], "public");
    assert_ne!(published.headers["etag"], created.headers["etag"]);
    assert_eq!(published.body.as_object().unwrap().len(), 2);
    let visible = anonymous.call("GET", &resource, &[], None).await;
    assert_eq!(visible.status, StatusCode::OK);
    assert_eq!(visible.headers["etag"], published.headers["etag"]);
    assert_eq!(visible.body["title"], created.body["title"]);
    assert_eq!(visible.body["body"], created.body["body"]);
    let no_op = owner
        .call(
            "PUT",
            &publication,
            &[("if-match", published.headers["etag"].to_str().unwrap())],
            None,
        )
        .await;
    assert_eq!(no_op.status, StatusCode::OK);
    assert_eq!(no_op.headers["etag"], published.headers["etag"]);
    assert_eq!(
        owner
            .call("PUT", &publication, &[("if-match", initial_tag)], None)
            .await
            .status,
        StatusCode::PRECONDITION_FAILED
    );
    assert_eq!(
        owner
            .call(
                "PATCH",
                &resource,
                &[("if-match", initial_tag)],
                Some(json!({"title":"stale"}))
            )
            .await
            .status,
        StatusCode::PRECONDITION_FAILED
    );
    for _ in 0..2 {
        assert_eq!(
            administrator
                .call("DELETE", &publication, &[], None)
                .await
                .status,
            StatusCode::NO_CONTENT
        );
    }
    assert_eq!(
        anonymous.call("GET", &resource, &[], None).await.status,
        StatusCode::NOT_FOUND
    );
    let hidden = owner.call("GET", &resource, &[], None).await;
    assert_ne!(hidden.headers["etag"], published.headers["etag"]);
    assert_eq!(hidden.body["body"], created.body["body"]);
    // Removing the editing assignment does not change who originally posted this editorial.
    assert_eq!(
        administrator
            .call(
                "DELETE",
                &format!("{resource}/authors/{}", own.id),
                &[],
                None
            )
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    let republished = administrator
        .call(
            "PUT",
            &publication,
            &[("if-match", hidden.headers["etag"].to_str().unwrap())],
            None,
        )
        .await;
    assert_eq!(republished.status, StatusCode::OK);
    assert_eq!(
        owner.call("DELETE", &resource, &[], None).await.status,
        StatusCode::NO_CONTENT
    );
    for client in [&anonymous, &owner, &administrator] {
        assert_eq!(
            client.call("GET", &resource, &[], None).await.status,
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        owner.create(problem, "lifecycle").await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        owner
            .call(
                "PATCH",
                &resource,
                &[("if-match", republished.headers["etag"].to_str().unwrap())],
                Some(json!({"title":"resurrect"}))
            )
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        administrator
            .call(
                "PUT",
                &publication,
                &[("if-match", republished.headers["etag"].to_str().unwrap())],
                None
            )
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    let listed = anonymous
        .call(
            "GET",
            &format!("/api/v1/problems/{problem}/editorials"),
            &[],
            None,
        )
        .await;
    assert!(listed.body["items"].as_array().unwrap().is_empty());
    let second = owner.create(problem, "admin-delete").await;
    assert_eq!(
        administrator
            .call("DELETE", &path(&second), &[], None)
            .await
            .status,
        StatusCode::NO_CONTENT
    );
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn problem_deletion_removes_its_editorials_in_the_same_transaction(pool: PgPool) {
    let own = actor(&pool, Some("standard-user")).await;
    let problems = Problems::new(&pool);
    let problem = problems.public().await;
    let another = problems.public().await;
    let router = app(&pool);
    let client = Client::new(&router, Some(&own));
    let first = client.create(problem, "parent-1").await;
    let second = client.create(problem, "parent-2").await;
    let unrelated = client.create(another, "different-parent").await;
    for commit in [false, true] {
        let mut tx = pool.begin().await.unwrap();
        sqlx::query("UPDATE problems SET deleted_at = clock_timestamp() WHERE id = $1")
            .bind(problem)
            .execute(&mut *tx)
            .await
            .unwrap();
        if commit {
            tx.commit().await.unwrap();
        } else {
            tx.rollback().await.unwrap();
        }
        let deleted: bool =
            sqlx::query_scalar("SELECT deleted_at IS NOT NULL FROM problems WHERE id = $1")
                .bind(problem)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(deleted, commit);
        for resource in [&first, &second] {
            assert_eq!(
                client.call("GET", &path(resource), &[], None).await.status,
                if commit {
                    StatusCode::NOT_FOUND
                } else {
                    StatusCode::OK
                }
            );
        }
        assert_eq!(
            client
                .call("GET", &path(&unrelated), &[], None)
                .await
                .status,
            StatusCode::OK
        );
    }
    let active: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM editorials WHERE problem_id = $1 AND deleted_at IS NULL",
    )
    .bind(problem)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(active, 0);
    assert_eq!(
        client
            .call(
                "GET",
                &format!("/api/v1/problems/{problem}/editorials"),
                &[],
                None
            )
            .await
            .status,
        StatusCode::NOT_FOUND
    );
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn private_problem_roles_are_checked_against_the_exact_problem(pool: PgPool) {
    let own = actor(&pool, Some("standard-user")).await;
    let tester = actor(&pool, Some("standard-user")).await;
    let unrelated_owner = actor(&pool, Some("standard-user")).await;
    let admin = actor(&pool, Some("service-admin")).await;
    let problems = Problems::new(&pool);
    let private = problems.private(own.session.user_id).await;
    problems.private(unrelated_owner.session.user_id).await;
    let router = app(&pool);
    let owner = Client::new(&router, Some(&own));
    let tester_client = Client::new(&router, Some(&tester));
    assert_eq!(
        Client::new(&router, Some(&unrelated_owner))
            .create(private, "unrelated")
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        tester_client.create(private, "before-role").await.status,
        StatusCode::NOT_FOUND
    );
    sqlx::query("INSERT INTO account_roles (user_id,role,scope_kind,scope_id) VALUES ($1,'tester','problem',$2)")
        .bind(tester.id).bind(private).execute(&pool).await.unwrap();
    let created = tester_client.create(private, "after-role").await;
    assert_eq!(created.status, StatusCode::CREATED);
    let publication = format!("{}/publication", path(&created));
    assert_eq!(
        tester_client
            .call(
                "PUT",
                &publication,
                &[("if-match", created.headers["etag"].to_str().unwrap())],
                None
            )
            .await
            .status,
        StatusCode::OK
    );
    assert_eq!(
        owner.call("GET", &path(&created), &[], None).await.status,
        StatusCode::OK
    );
    assert_eq!(
        Client::new(&router, None)
            .call("GET", &path(&created), &[], None)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        Client::new(&router, Some(&admin))
            .call("GET", &path(&created), &[], None)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    sqlx::query("DELETE FROM account_roles WHERE user_id = $1 AND role = 'tester'")
        .bind(tester.id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        tester_client
            .call("GET", &path(&created), &[], None)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    let current = owner.call("GET", &path(&created), &[], None).await;
    assert_eq!(
        tester_client
            .call(
                "PATCH",
                &path(&created),
                &[("if-match", current.headers["etag"].to_str().unwrap())],
                Some(json!({"title":"denied after role removal"}))
            )
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    let error=sqlx::query("INSERT INTO editorials (id,problem_id,author_id,title,body,visibility,revision,created_at,updated_at) VALUES ($1,$2,$3,'title','body','private','v1',clock_timestamp(),clock_timestamp())")
        .bind(Uuid::new_v4()).bind(Uuid::new_v4()).bind(own.id).execute(&pool).await.unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("23503")
    );
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn concurrent_problem_deletion_and_editorial_update_leave_no_active_editorial(pool: PgPool) {
    let own = actor(&pool, Some("standard-user")).await;
    let problems = Problems::new(&pool);
    let problem = problems.public().await;
    let router = app(&pool);
    let client = Client::new(&router, Some(&own));
    let created = client.create(problem, "concurrent-parent-deletion").await;
    let resource = path(&created);
    let headers = [("if-match", created.headers["etag"].to_str().unwrap())];
    let deletion = async {
        let mut tx = pool.begin().await.unwrap();
        sqlx::query("SET LOCAL lock_timeout = '3s'")
            .execute(&mut *tx)
            .await
            .unwrap();
        sqlx::query("UPDATE problems SET deleted_at = clock_timestamp() WHERE id = $1")
            .bind(problem)
            .execute(&mut *tx)
            .await
            .unwrap();
        tx.commit().await.unwrap();
    };
    let (updated, ()) = tokio::join!(
        client.call(
            "PATCH",
            &resource,
            &headers,
            Some(json!({"title":"concurrent edit"}))
        ),
        deletion
    );
    assert!(
        [StatusCode::OK, StatusCode::NOT_FOUND].contains(&updated.status),
        "{}: {}",
        updated.status,
        updated.body
    );
    assert_eq!(
        client.call("GET", &resource, &[], None).await.status,
        StatusCode::NOT_FOUND
    );
    let active: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM editorials WHERE problem_id = $1 AND deleted_at IS NULL",
    )
    .bind(problem)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(active, 0);
}
