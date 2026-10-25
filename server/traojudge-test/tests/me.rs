use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{HeaderMap, Request, StatusCode},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Duration, Utc};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tower::ServiceExt;
use traojudge_backend_app::http::{build_router, middleware::origin::WebOrigin};
use traojudge_core::{
    application::{account::AccountService, session::SessionService, users::GetUserProfile},
    domain::model::{account::*, user::UserId},
    ports::{
        account_services::AccountExternalServices,
        repository::account::{AccountUnitOfWork, AccountUnitOfWorkProvider, SessionRepository},
    },
};
use traojudge_infra::{
    account_services::UnconfiguredAccountServices,
    postgres::{
        accounts::PgAccountUnitOfWorkProvider, sessions::PgSessionRepository,
        users::PgUserProfileRepository,
    },
};
use url::Url;
use uuid::Uuid;

const ORIGIN: &str = "https://judge.example";

#[derive(Clone, Default)]
struct External(Arc<Mutex<ExternalState>>);
#[derive(Default)]
struct ExternalState {
    emails: Vec<(String, String)>,
    codes: HashMap<String, (Provider, String, String)>,
    fail_delivery: bool,
}

#[async_trait]
impl AccountExternalServices for External {
    fn mail_available(&self) -> bool {
        true
    }
    async fn send_email_change(&self, email: &str, token: &str) -> Result<(), AccountError> {
        let mut state = self.0.lock().unwrap();
        if state.fail_delivery {
            return Err(AccountError::Unavailable);
        }
        state.emails.push((email.to_owned(), token.to_owned()));
        Ok(())
    }
    async fn authorization_url(
        &self,
        provider: Provider,
        state: &str,
        challenge: &str,
    ) -> Result<String, AccountError> {
        Ok(format!(
            "https://provider.example/{}/authorize?state={state}&code_challenge={challenge}",
            provider.as_str()
        ))
    }
    async fn verify_authorization(
        &self,
        provider: Provider,
        code: &str,
        verifier: &str,
    ) -> Result<String, AccountError> {
        let state = self.0.lock().unwrap();
        let (expected_provider, subject, challenge) =
            state.codes.get(code).ok_or(AccountError::Unavailable)?;
        if provider != *expected_provider
            || URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())) != *challenge
        {
            return Err(AccountError::Unavailable);
        }
        Ok(subject.to_owned())
    }
}

struct Session {
    id: Uuid,
    secret: String,
}

async fn user(pool: &PgPool) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, display_name, revision, github_id) VALUES ($1, '利用者', 'initial', 'old-github')")
        .bind(id).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO account_roles (user_id, role, scope_kind) VALUES ($1, 'standard-user', 'service')")
        .bind(id).execute(pool).await.unwrap();
    id
}

async fn session(pool: &PgPool, user_id: Uuid) -> Session {
    let secret = Secret::random();
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO web_sessions (id, user_id, token_digest, expires_at) VALUES ($1, $2, $3, clock_timestamp() + INTERVAL '1 day')")
        .bind(id).bind(user_id).bind(secret.digest()).execute(pool).await.unwrap();
    Session {
        id,
        secret: secret.expose().to_owned(),
    }
}

async fn method(pool: &PgPool, user_id: Uuid, kind: &str, subject: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO sign_in_methods (user_id, kind, subject) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(user_id)
    .bind(kind)
    .bind(subject)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn app(pool: &PgPool, external: External) -> Router {
    build_router(
        GetUserProfile::new(PgUserProfileRepository::new(pool.clone())),
        AccountService::new(PgAccountUnitOfWorkProvider::new(pool.clone()), external),
        SessionService::new(PgSessionRepository::new(pool.clone())),
        WebOrigin::parse(ORIGIN).unwrap(),
    )
}

async fn request(
    router: &Router,
    method: &str,
    path: &str,
    secret: Option<&str>,
    origin: Option<&str>,
    etag: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, HeaderMap, Value) {
    let mut builder = Request::builder().method(method).uri(path);
    if let Some(secret) = secret {
        builder = builder.header("cookie", format!("traojudge_session={secret}"));
    }
    if let Some(origin) = origin {
        builder = builder.header("origin", origin);
    }
    if let Some(etag) = etag {
        builder = builder.header("if-match", etag);
    }
    let body = if let Some(body) = body {
        builder = builder.header("content-type", "application/json");
        Body::from(body.to_string())
    } else {
        Body::empty()
    };
    let response = router
        .clone()
        .oneshot(builder.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    if path.starts_with("/api/v1/me") {
        assert_eq!(headers["cache-control"], "no-store");
    }
    let bytes = to_bytes(response.into_body(), 128 * 1024).await.unwrap();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, headers, body)
}

async fn start(
    router: &Router,
    session: &Session,
    provider: &str,
    external: &External,
    code: &str,
    subject: &str,
) -> (String, String) {
    let (status, _, body) = request(
        router,
        "POST",
        "/api/v1/me/sign-in-methods/external-authorizations",
        Some(&session.secret),
        Some(ORIGIN),
        None,
        Some(json!({"provider": provider})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let url = Url::parse(body["authorizationUrl"].as_str().unwrap()).unwrap();
    let query = url.query_pairs().collect::<HashMap<_, _>>();
    let state = query["state"].to_string();
    external.0.lock().unwrap().codes.insert(
        code.to_owned(),
        (
            provider.parse().unwrap(),
            subject.to_owned(),
            query["code_challenge"].to_string(),
        ),
    );
    (
        format!(
            "/api/v1/me/sign-in-methods/external-authorizations/{}/complete",
            body["authorizationId"].as_str().unwrap()
        ),
        state,
    )
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn session_authentication_and_profile_updates(pool: PgPool) {
    let id = user(&pool).await;
    let own = session(&pool, id).await;
    let other_id = user(&pool).await;
    let expired = session(&pool, id).await;
    let revoked = session(&pool, id).await;
    sqlx::query("UPDATE web_sessions SET expires_at = clock_timestamp() - INTERVAL '1 second' WHERE id = $1").bind(expired.id).execute(&pool).await.unwrap();
    sqlx::query("UPDATE web_sessions SET revoked_at = clock_timestamp() WHERE id = $1")
        .bind(revoked.id)
        .execute(&pool)
        .await
        .unwrap();
    let router = app(&pool, External::default());

    for secret in [
        None,
        Some(expired.secret.as_str()),
        Some(revoked.secret.as_str()),
        Some("not-a-session"),
        Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
    ] {
        let (status, headers, _) =
            request(&router, "GET", "/api/v1/me", secret, None, None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(!headers.contains_key("set-cookie"));
    }
    for origin in [
        None,
        Some("null"),
        Some("https://evil.example"),
        Some("https://judge.example:444"),
        Some("https://judge.example/evil"),
    ] {
        let (status, _, _) = request(
            &router,
            "PATCH",
            "/api/v1/me",
            Some(&own.secret),
            origin,
            None,
            Some(json!({"bio":"bad"})),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }
    let (status, headers, body) = request(
        &router,
        "GET",
        "/api/v1/me",
        Some(&own.secret),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["id"], id.to_string());
    assert_eq!(body["roles"][0]["scope"], json!({"kind":"service"}));
    assert_eq!(body["roles"][0]["userId"], id.to_string());
    let cookie = headers["set-cookie"].to_str().unwrap();
    for flag in [
        "HttpOnly",
        "Secure",
        "SameSite=Lax",
        "Max-Age=7776000",
        "Path=/",
    ] {
        assert!(cookie.contains(flag));
    }
    let expires: DateTime<Utc> =
        sqlx::query_scalar("SELECT expires_at FROM web_sessions WHERE id = $1")
            .bind(own.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(expires > Utc::now() + Duration::days(89));
    let tag = headers["etag"].to_str().unwrap();

    // 構文上正しい未知の強いETagは、更新競合として扱う。
    assert_eq!(
        request(
            &router,
            "PATCH",
            "/api/v1/me",
            Some(&own.secret),
            Some(ORIGIN),
            Some("\"stale\""),
            Some(json!({"bio":"new"}))
        )
        .await
        .0,
        StatusCode::PRECONDITION_FAILED
    );
    let duplicate_cookie = Request::builder()
        .uri("/api/v1/me")
        .header(
            "cookie",
            format!("traojudge_session={0}; traojudge_session={0}", own.secret),
        )
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        router
            .clone()
            .oneshot(duplicate_cookie)
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );

    let (status, _, _) = request(
        &router,
        "PATCH",
        "/api/v1/me",
        Some(&own.secret),
        Some(ORIGIN),
        None,
        Some(json!({"bio":"new"})),
    )
    .await;
    assert_eq!(status, StatusCode::PRECONDITION_REQUIRED);
    for invalid in [
        json!({}),
        json!({"displayName":""}),
        json!({"displayName":"あ".repeat(81)}),
        json!({"bio":"あ".repeat(4001)}),
        json!({"bio":null}),
        json!({"roles":[]}),
        json!({"id":other_id}),
        json!({"githubId":42}),
    ] {
        let (status, _, _) = request(
            &router,
            "PATCH",
            "/api/v1/me",
            Some(&own.secret),
            Some(ORIGIN),
            Some(tag),
            Some(invalid),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }
    for invalid_tag in ["*", "W/\"initial\"", "unquoted"] {
        assert_eq!(
            request(
                &router,
                "PATCH",
                "/api/v1/me",
                Some(&own.secret),
                Some(ORIGIN),
                Some(invalid_tag),
                Some(json!({"bio":"new"}))
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
    let first = request(
        &router,
        "PATCH",
        "/api/v1/me",
        Some(&own.secret),
        Some(ORIGIN),
        Some(tag),
        Some(json!({"bio":"first", "githubId":""})),
    );
    let second = request(
        &router,
        "PATCH",
        "/api/v1/me",
        Some(&own.secret),
        Some(ORIGIN),
        Some(tag),
        Some(json!({"bio":"second", "githubId":""})),
    );
    let (first, second) = tokio::join!(first, second);
    let statuses = [first.0, second.0];
    assert_eq!(statuses.iter().filter(|&&s| s == StatusCode::OK).count(), 1);
    assert_eq!(
        statuses
            .iter()
            .filter(|&&s| s == StatusCode::PRECONDITION_FAILED)
            .count(),
        1
    );
    let (_, new_headers, new_body) = request(
        &router,
        "GET",
        "/api/v1/me",
        Some(&own.secret),
        None,
        None,
        None,
    )
    .await;
    assert_ne!(new_headers["etag"], headers["etag"]);
    assert_eq!(new_body["displayName"], "利用者");
    assert_eq!(new_body["githubId"], "");
    let before_roles = new_headers["etag"].clone();
    sqlx::query("INSERT INTO account_roles (user_id, role, scope_kind, scope_id) VALUES ($1, 'tester', 'problem', $2)").bind(id).bind(Uuid::new_v4()).execute(&pool).await.unwrap();
    let (_, role_headers, role_body) = request(
        &router,
        "GET",
        "/api/v1/me",
        Some(&own.secret),
        None,
        None,
        None,
    )
    .await;
    assert_ne!(role_headers["etag"], before_roles);
    let (_, _, roles) = request(
        &router,
        "GET",
        "/api/v1/me/roles",
        Some(&own.secret),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(roles["items"], role_body["roles"]);
    let (_, _, public) = request(
        &router,
        "GET",
        &format!("/api/v1/users/{id}"),
        Some(&own.secret),
        None,
        None,
        None,
    )
    .await;
    assert!(public.get("roles").is_none());
    assert!(public.get("email").is_none());
    assert_eq!(public["bio"], new_body["bio"]);
    let unchanged: String = sqlx::query_scalar("SELECT bio FROM users WHERE id = $1")
        .bind(other_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(unchanged, "");
    let (status, _, valid_unicode) = request(
        &router,
        "PATCH",
        "/api/v1/me",
        Some(&own.secret),
        Some(ORIGIN),
        role_headers["etag"].to_str().ok(),
        Some(json!({"displayName":"あ".repeat(80), "bio":"あ".repeat(4000)})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        valid_unicode["displayName"]
            .as_str()
            .unwrap()
            .chars()
            .count(),
        80
    );
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn competing_email_changes_do_not_move_another_identity(pool: PgPool) {
    let first_user = user(&pool).await;
    let second_user = user(&pool).await;
    let first = session(&pool, first_user).await;
    let second = session(&pool, second_user).await;
    let first_extra = session(&pool, first_user).await;
    let second_extra = session(&pool, second_user).await;
    method(&pool, first_user, "email", "first@example.com").await;
    method(&pool, second_user, "email", "second@example.com").await;
    let external = External::default();
    let router = app(&pool, external.clone());
    for own in [&first, &second] {
        assert_eq!(
            request(
                &router,
                "POST",
                "/api/v1/me/email-changes",
                Some(&own.secret),
                Some(ORIGIN),
                None,
                Some(json!({"email":"contended@example.com"}))
            )
            .await
            .0,
            StatusCode::ACCEPTED
        );
    }
    let (first_token, second_token) = {
        let state = external.0.lock().unwrap();
        (state.emails[0].1.clone(), state.emails[1].1.clone())
    };
    let (a, b) = tokio::join!(
        request(
            &router,
            "POST",
            "/api/v1/me/email-changes/complete",
            Some(&first.secret),
            Some(ORIGIN),
            None,
            Some(json!({"token":first_token}))
        ),
        request(
            &router,
            "POST",
            "/api/v1/me/email-changes/complete",
            Some(&second.secret),
            Some(ORIGIN),
            None,
            Some(json!({"token":second_token}))
        ),
    );
    let statuses = [a.0, b.0];
    assert_eq!(
        statuses
            .iter()
            .filter(|&&s| s == StatusCode::NO_CONTENT)
            .count(),
        1
    );
    assert_eq!(
        statuses
            .iter()
            .filter(|&&s| s == StatusCode::CONFLICT)
            .count(),
        1
    );
    let (loser_user, loser_extra, loser_token, old_email) = if a.0 == StatusCode::CONFLICT {
        (first_user, first_extra, first_token, "first@example.com")
    } else {
        (
            second_user,
            second_extra,
            second_token,
            "second@example.com",
        )
    };
    let email: String = sqlx::query_scalar(
        "SELECT subject FROM sign_in_methods WHERE user_id = $1 AND kind = 'email'",
    )
    .bind(loser_user)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(email, old_email);
    let unused: bool =
        sqlx::query_scalar("SELECT consumed_at IS NULL FROM email_changes WHERE token_digest = $1")
            .bind(token_digest(&loser_token))
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(unused);
    assert_eq!(
        request(
            &router,
            "GET",
            "/api/v1/me",
            Some(&loser_extra.secret),
            None,
            None,
            None
        )
        .await
        .0,
        StatusCode::OK
    );
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn developer_token_lifecycle_and_ownership(pool: PgPool) {
    let id = user(&pool).await;
    let own = session(&pool, id).await;
    let other = session(&pool, user(&pool).await).await;
    let router = app(&pool, External::default());
    let (status, _, body) = request(
        &router,
        "POST",
        "/api/v1/me/developer-tokens",
        Some(&own.secret),
        Some(ORIGIN),
        None,
        Some(json!({"name":"CLI"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert!(body["token"]["expiresAt"].is_null());
    let secret = body["secret"].as_str().unwrap();
    let token_id = Uuid::parse_str(body["token"]["id"].as_str().unwrap()).unwrap();
    let repository = PgSessionRepository::new(pool.clone());
    assert_eq!(
        repository
            .developer_token_user(&token_digest(secret))
            .await
            .unwrap(),
        Some(UserId::new(id))
    );
    let digest: Vec<u8> =
        sqlx::query_scalar("SELECT token_digest FROM developer_tokens WHERE id = $1")
            .bind(token_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(digest, token_digest(secret));
    let (_, _, listed) = request(
        &router,
        "GET",
        "/api/v1/me/developer-tokens",
        Some(&own.secret),
        None,
        None,
        None,
    )
    .await;
    assert!(!listed.to_string().contains(secret));
    assert!(listed["items"][0].get("secret").is_none());
    let (_, _, others) = request(
        &router,
        "GET",
        "/api/v1/me/developer-tokens",
        Some(&other.secret),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(others["items"], json!([]));
    assert_eq!(
        request(&router, "GET", "/api/v1/me", Some(secret), None, None, None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let path = format!("/api/v1/me/developer-tokens/{token_id}");
    assert_eq!(
        request(
            &router,
            "DELETE",
            &path,
            Some(&other.secret),
            Some(ORIGIN),
            None,
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    for _ in 0..2 {
        assert_eq!(
            request(
                &router,
                "DELETE",
                &path,
                Some(&own.secret),
                Some(ORIGIN),
                None,
                None
            )
            .await
            .0,
            StatusCode::NO_CONTENT
        );
    }
    assert!(
        repository
            .developer_token_user(&token_digest(secret))
            .await
            .unwrap()
            .is_none()
    );
    for input in [
        json!({"name":""}),
        json!({"name":"x", "expiresAt":"2000-01-01T00:00:00Z"}),
        json!({"name":"x", "expiresAt":null}),
    ] {
        assert_eq!(
            request(
                &router,
                "POST",
                "/api/v1/me/developer-tokens",
                Some(&own.secret),
                Some(ORIGIN),
                None,
                Some(input)
            )
            .await
            .0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    let (status, _, expiring) = request(
        &router,
        "POST",
        "/api/v1/me/developer-tokens",
        Some(&own.secret),
        Some(ORIGIN),
        None,
        Some(json!({"name":"temporary", "expiresAt":(Utc::now()+Duration::days(1)).to_rfc3339()})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let expiring_id = Uuid::parse_str(expiring["token"]["id"].as_str().unwrap()).unwrap();
    sqlx::query("UPDATE developer_tokens SET expires_at = clock_timestamp() - INTERVAL '1 second' WHERE id = $1").bind(expiring_id).execute(&pool).await.unwrap();
    assert!(
        repository
            .developer_token_user(&token_digest(expiring["secret"].as_str().unwrap()))
            .await
            .unwrap()
            .is_none()
    );
    let (_, _, me) = request(
        &router,
        "GET",
        "/api/v1/me",
        Some(&own.secret),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(me["roles"].as_array().unwrap().len(), 1);
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn email_change_privacy_expiration_and_session_revocation(pool: PgPool) {
    let id = user(&pool).await;
    let own = session(&pool, id).await;
    let second = session(&pool, id).await;
    let other_id = user(&pool).await;
    let other = session(&pool, other_id).await;
    method(&pool, id, "email", "old@example.com").await;
    method(&pool, other_id, "email", "used@example.com").await;
    let external = External::default();
    let router = app(&pool, external.clone());
    let path = "/api/v1/me/email-changes";
    assert_eq!(
        request(
            &router,
            "POST",
            path,
            Some(&own.secret),
            Some(ORIGIN),
            None,
            Some(json!({"email":"new@example.com"}))
        )
        .await
        .0,
        StatusCode::ACCEPTED
    );
    let old_token = external.0.lock().unwrap().emails[0].1.clone();
    let (_, _, accepted) = request(
        &router,
        "POST",
        path,
        Some(&own.secret),
        Some(ORIGIN),
        None,
        Some(json!({"email":"used@example.com"})),
    )
    .await;
    assert_eq!(accepted.as_object().unwrap().len(), 2);
    assert_eq!(external.0.lock().unwrap().emails.len(), 1);
    let still_valid: bool =
        sqlx::query_scalar("SELECT consumed_at IS NULL FROM email_changes WHERE token_digest = $1")
            .bind(token_digest(&old_token))
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(still_valid);
    assert_eq!(
        request(
            &router,
            "POST",
            path,
            Some(&own.secret),
            Some(ORIGIN),
            None,
            Some(json!({"email":"latest@example.com"}))
        )
        .await
        .0,
        StatusCode::ACCEPTED
    );
    let token = external.0.lock().unwrap().emails[1].1.clone();
    let complete = "/api/v1/me/email-changes/complete";
    for (secret, token) in [(&own.secret, &old_token), (&other.secret, &token)] {
        assert_eq!(
            request(
                &router,
                "POST",
                complete,
                Some(secret),
                Some(ORIGIN),
                None,
                Some(json!({"token":token}))
            )
            .await
            .0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    assert_eq!(
        request(
            &router,
            "GET",
            "/api/v1/me",
            Some(&second.secret),
            None,
            None,
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, headers, _) = request(
        &router,
        "POST",
        complete,
        Some(&own.secret),
        Some(ORIGIN),
        None,
        Some(json!({"token":token})),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(headers.contains_key("set-cookie"));
    assert_eq!(
        request(
            &router,
            "GET",
            "/api/v1/me",
            Some(&own.secret),
            None,
            None,
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        request(
            &router,
            "GET",
            "/api/v1/me",
            Some(&second.secret),
            None,
            None,
            None
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(
            &router,
            "GET",
            "/api/v1/me",
            Some(&other.secret),
            None,
            None,
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        request(
            &router,
            "POST",
            complete,
            Some(&own.secret),
            Some(ORIGIN),
            None,
            Some(json!({"token":token}))
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let email: String = sqlx::query_scalar(
        "SELECT subject FROM sign_in_methods WHERE user_id = $1 AND kind = 'email'",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(email, "latest@example.com");
    request(
        &router,
        "POST",
        path,
        Some(&own.secret),
        Some(ORIGIN),
        None,
        Some(json!({"email":"expired@example.com"})),
    )
    .await;
    let expired_token = external.0.lock().unwrap().emails[2].1.clone();
    sqlx::query("UPDATE email_changes SET expires_at = clock_timestamp() WHERE token_digest = $1")
        .bind(token_digest(&expired_token))
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        request(
            &router,
            "POST",
            complete,
            Some(&own.secret),
            Some(ORIGIN),
            None,
            Some(json!({"token":expired_token}))
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    external.0.lock().unwrap().fail_delivery = true;
    for email in ["used@example.com", "delivery-failed@example.com"] {
        assert_eq!(
            request(
                &router,
                "POST",
                path,
                Some(&own.secret),
                Some(ORIGIN),
                None,
                Some(json!({"email":email}))
            )
            .await
            .0,
            StatusCode::ACCEPTED
        );
    }
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn external_methods_require_verified_identity_and_correct_browser(pool: PgPool) {
    let id = user(&pool).await;
    let own = session(&pool, id).await;
    let same_user_browser = session(&pool, id).await;
    let other_id = user(&pool).await;
    let other = session(&pool, other_id).await;
    let email_id = method(&pool, id, "email", "self@example.com").await;
    let foreign_id = method(&pool, other_id, "github", "foreign-github").await;
    let external = External::default();
    let router = app(&pool, external.clone());
    let (path, state) = start(
        &router,
        &own,
        "google",
        &external,
        "verified-google-code",
        "google-subject",
    )
    .await;
    for (secret, supplied_state) in [
        (&own.secret, "wrong"),
        (&same_user_browser.secret, state.as_str()),
        (&other.secret, state.as_str()),
    ] {
        assert_eq!(
            request(
                &router,
                "POST",
                &path,
                Some(secret),
                Some(ORIGIN),
                None,
                Some(json!({"code":"verified-google-code", "state":supplied_state}))
            )
            .await
            .0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    assert_eq!(
        request(
            &router,
            "POST",
            &path,
            Some(&own.secret),
            Some(ORIGIN),
            None,
            Some(json!({"code":"unverified-code", "state":state}))
        )
        .await
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    let (status, _, linked) = request(
        &router,
        "POST",
        &path,
        Some(&own.secret),
        Some(ORIGIN),
        None,
        Some(json!({"code":"verified-google-code", "state":state})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, _, repeated) = request(
        &router,
        "POST",
        &path,
        Some(&own.secret),
        Some(ORIGIN),
        None,
        Some(json!({"code":"verified-google-code", "state":state})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(linked, repeated);
    assert_eq!(
        request(
            &router,
            "POST",
            "/api/v1/me/sign-in-methods/external-authorizations",
            Some(&own.secret),
            Some(ORIGIN),
            None,
            Some(json!({"provider":"google"}))
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let (foreign_path, foreign_state) = start(
        &router,
        &own,
        "github",
        &external,
        "verified-foreign-code",
        "foreign-github",
    )
    .await;
    assert_eq!(
        request(
            &router,
            "POST",
            &foreign_path,
            Some(&own.secret),
            Some(ORIGIN),
            None,
            Some(json!({"code":"verified-foreign-code", "state":foreign_state}))
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    for target in [email_id.to_string(), foreign_id.to_string()] {
        assert_eq!(
            request(
                &router,
                "DELETE",
                &format!("/api/v1/me/sign-in-methods/{target}"),
                Some(&own.secret),
                Some(ORIGIN),
                None,
                None
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
    }
    let removal = format!(
        "/api/v1/me/sign-in-methods/{}",
        linked["id"].as_str().unwrap()
    );
    assert_eq!(
        request(
            &router,
            "DELETE",
            &removal,
            Some(&own.secret),
            Some(ORIGIN),
            None,
            None
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        request(
            &router,
            "POST",
            &path,
            Some(&own.secret),
            Some(ORIGIN),
            None,
            Some(json!({"code":"verified-google-code", "state":state}))
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let (_, _, methods) = request(
        &router,
        "GET",
        "/api/v1/me/sign-in-methods",
        Some(&own.secret),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(methods["items"].as_array().unwrap().len(), 1);
    assert!(methods["items"][0].get("subject").is_none());
    let (_, _, roles) = request(
        &router,
        "GET",
        "/api/v1/me/roles",
        Some(&own.secret),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(roles["items"].as_array().unwrap().len(), 1);
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn concurrent_removal_keeps_a_web_login_method(pool: PgPool) {
    let id = user(&pool).await;
    let own = session(&pool, id).await;
    let google = method(&pool, id, "google", "google-subject").await;
    let github = method(&pool, id, "github", "github-subject").await;
    let router = app(&pool, External::default());
    let first_path = format!("/api/v1/me/sign-in-methods/{google}");
    let second_path = format!("/api/v1/me/sign-in-methods/{github}");
    let (first, second) = tokio::join!(
        request(
            &router,
            "DELETE",
            &first_path,
            Some(&own.secret),
            Some(ORIGIN),
            None,
            None
        ),
        request(
            &router,
            "DELETE",
            &second_path,
            Some(&own.secret),
            Some(ORIGIN),
            None,
            None
        ),
    );
    let statuses = [first.0, second.0];
    assert_eq!(
        statuses
            .iter()
            .filter(|&&s| s == StatusCode::NO_CONTENT)
            .count(),
        1
    );
    assert_eq!(
        statuses
            .iter()
            .filter(|&&s| s == StatusCode::CONFLICT)
            .count(),
        1
    );
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn unconfigured_external_connections_fail_explicitly(pool: PgPool) {
    let own = session(&pool, user(&pool).await).await;
    let router = build_router(
        GetUserProfile::new(PgUserProfileRepository::new(pool.clone())),
        AccountService::new(
            PgAccountUnitOfWorkProvider::new(pool.clone()),
            UnconfiguredAccountServices,
        ),
        SessionService::new(PgSessionRepository::new(pool)),
        WebOrigin::parse(ORIGIN).unwrap(),
    );
    for (path, input) in [
        (
            "/api/v1/me/email-changes",
            json!({"email":"user@example.com"}),
        ),
        (
            "/api/v1/me/sign-in-methods/external-authorizations",
            json!({"provider":"google"}),
        ),
    ] {
        assert_eq!(
            request(
                &router,
                "POST",
                path,
                Some(&own.secret),
                Some(ORIGIN),
                None,
                Some(input)
            )
            .await
            .0,
            StatusCode::SERVICE_UNAVAILABLE
        );
    }
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn unit_of_work_commits_together_and_rolls_back_unfinished_changes(pool: PgPool) {
    let id = user(&pool).await;
    let own = session(&pool, id).await;
    let actor = AuthenticatedSession {
        id: own.id,
        user_id: UserId::new(id),
    };
    let provider = PgAccountUnitOfWorkProvider::new(pool.clone());

    for outcome in ["drop", "rollback", "commit"] {
        let mut tx = provider.begin(actor).await.unwrap();
        let mut profile = tx.current_account().await.unwrap().profile;
        profile.display_name = outcome.to_owned();
        tx.save_profile(&profile).await.unwrap();
        let token_id = Uuid::new_v4();
        tx.insert_developer_token(NewDeveloperToken {
            id: token_id,
            name: outcome.to_owned(),
            digest: Secret::random().digest(),
            expires_at: None,
        })
        .await
        .unwrap();
        match outcome {
            "commit" => tx.commit().await.unwrap(),
            "rollback" => tx.rollback().await.unwrap(),
            _ => drop(tx),
        }
        // Both writes are visible together, or neither survives the Unit of Work.
        let name: String = sqlx::query_scalar("SELECT display_name FROM users WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM developer_tokens WHERE id = $1)")
                .bind(token_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(
            name,
            if outcome == "commit" {
                "commit"
            } else {
                "利用者"
            }
        );
        assert_eq!(exists, outcome == "commit");
    }

    sqlx::query("UPDATE web_sessions SET revoked_at = clock_timestamp() WHERE id = $1")
        .bind(own.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        provider.begin(actor).await,
        Err(AccountError::Unauthenticated)
    ));
}

#[sqlx::test(migrator = "traojudge_infra::postgres::MIGRATOR")]
#[ignore = "requires DATABASE_URL with PostgreSQL CREATE DATABASE permission"]
async fn nested_routes_apply_authentication_and_keep_ping_independent(pool: PgPool) {
    let router = app(&pool, External::default());
    let resource = Uuid::new_v4();
    for (verb, path) in [
        ("GET", "/me".to_owned()),
        ("PATCH", "/me".to_owned()),
        ("GET", "/me/roles".to_owned()),
        ("GET", "/me/sign-in-methods".to_owned()),
        ("DELETE", format!("/me/sign-in-methods/{resource}")),
        (
            "POST",
            "/me/sign-in-methods/external-authorizations".to_owned(),
        ),
        (
            "POST",
            format!("/me/sign-in-methods/external-authorizations/{resource}/complete"),
        ),
        ("GET", "/me/developer-tokens".to_owned()),
        ("POST", "/me/developer-tokens".to_owned()),
        ("DELETE", format!("/me/developer-tokens/{resource}")),
        ("POST", "/me/email-changes".to_owned()),
        ("POST", "/me/email-changes/complete".to_owned()),
    ] {
        let (status, _, _) = request(
            &router,
            verb,
            &format!("/api/v1{path}"),
            None,
            Some(ORIGIN),
            None,
            Some(json!({})),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{verb} {path}");
    }
    let response = router
        .oneshot(
            Request::builder()
                .uri("/ping")
                .header("origin", "null")
                .header(
                    "cookie",
                    format!("traojudge_session={}", Secret::random().expose()),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!response.headers().contains_key("set-cookie"));
    assert_eq!(to_bytes(response.into_body(), 1024).await.unwrap(), "pong");
}
