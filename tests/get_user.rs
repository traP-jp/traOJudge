use std::borrow::BorrowMut;

use axum::{body::Body, http::Request};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::util::ServiceExt;
use trao_judge_backend::{make_router, Repository};

#[sqlx::test(fixtures("common"))]
async fn get_user_by_id(pool: sqlx::MySqlPool) -> anyhow::Result<()> {
    let state = Repository::create_by_pool(pool).await?;
    let mut app = make_router(state);

    let user_case = vec![
        (1, "test_user_1", "commonUser"),
        (2, "test_user_2", "traPUser"),
        (3, "test_user_3", "admin"),
    ];
    for (id, name, role) in user_case {
        let response = app
            .borrow_mut()
            .oneshot(
                Request::builder()
                    .extension("GET")
                    .uri(format!("/users/{}", id))
                    .body(Body::empty())?,
            )
            .await?;

        assert_eq!(response.status(), 200);

        let json: Value =
            serde_json::from_slice(&response.into_body().collect().await?.to_bytes())?;
        assert_eq!(json["name"], name);
        assert_eq!(json["role"], role);
    }

    Ok(())
}

#[sqlx::test(fixtures("common"))]
async fn get_user_by_id_not_found(pool: sqlx::MySqlPool) -> anyhow::Result<()> {
    let state = Repository::create_by_pool(pool).await?;
    let mut app = make_router(state);

    let not_found_case = vec![0, 4, 10, 1000000];
    for id in not_found_case {
        let response = app
            .borrow_mut()
            .oneshot(
                Request::builder()
                    .extension("GET")
                    .uri(format!("/users/{}", id))
                    .body(Body::empty())?,
            )
            .await?;

        assert_eq!(response.status(), 404);
    }

    Ok(())
}

#[sqlx::test(fixtures("common"))]
async fn get_user_me(pool: sqlx::MySqlPool) -> anyhow::Result<()> {
    let state = Repository::create_by_pool(pool).await?;
    let mut app = make_router(state.clone());

    let user_case = vec![
        (1, "test_user_1", "commonUser"),
        (2, "test_user_2", "traPUser"),
        (3, "test_user_3", "admin"),
    ];
    for (id, name, role) in user_case {
        let session_id = state
            .create_session(state.get_user_by_display_id(id).await?.unwrap())
            .await?;

        let response = app
            .borrow_mut()
            .oneshot(
                Request::builder()
                    .extension("GET")
                    .uri("/users/me")
                    .header("Cookie", format!("session_id={}", session_id))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(response.status(), 200);

        let json: Value =
            serde_json::from_slice(&response.into_body().collect().await?.to_bytes())?;
        assert_eq!(json["name"], name);
        assert_eq!(json["role"], role);
    }

    Ok(())
}

#[sqlx::test(fixtures("common"))]
async fn get_user_me_unauthorized(pool: sqlx::MySqlPool) -> anyhow::Result<()> {
    let state = Repository::create_by_pool(pool).await?;
    let mut app = make_router(state.clone());

    // Test unauthorized case
    let response = app
        .borrow_mut()
        .oneshot(
            Request::builder()
                .extension("GET")
                .uri("/users/me")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), 401);

    Ok(())
}
