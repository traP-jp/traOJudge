use std::borrow::BorrowMut;

use super::common::check::users_check_by_id;
use axum::{
    body::Body,
    http::{self, Request},
};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;
use trao_judge_backend::{
    di::DiContainer,
    domain::repository::{session::SessionRepository, user::UserRepository},
    infrastructure::provider::Provider,
    make_router,
};

#[sqlx::test(fixtures("common"))]
async fn get_user_by_id(pool: sqlx::MySqlPool) -> anyhow::Result<()> {
    let provider = Provider::create_by_pool(pool).await?;
    let di_container = DiContainer::new(provider).await;
    let mut app = make_router(di_container);

    let tests = vec![1, 2, 3];

    for id in tests {
        let response = app
            .borrow_mut()
            .oneshot(
                Request::builder()
                    .method(http::Method::GET)
                    .uri(format!("/users/{}", id))
                    .body(Body::empty())?,
            )
            .await?;

        assert_eq!(response.status(), 200);

        let mut resp_json: Value =
            serde_json::from_slice(&response.into_body().collect().await?.to_bytes())?;

        users_check_by_id(id, &mut resp_json)?;
    }

    Ok(())
}

#[sqlx::test(fixtures("common"))]
async fn get_user_by_id_not_found(pool: sqlx::MySqlPool) -> anyhow::Result<()> {
    let provider = Provider::create_by_pool(pool).await?;
    let di_container = DiContainer::new(provider).await;
    let mut app = make_router(di_container);

    let not_found_case = vec![0, 4, 10, 1000000];
    for id in not_found_case {
        let response = app
            .borrow_mut()
            .oneshot(
                Request::builder()
                    .method(http::Method::GET)
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
    let provider = Provider::create_by_pool(pool).await?;
    let di_container = DiContainer::new(provider.clone()).await;
    let mut app = make_router(di_container);

    let tests = vec![1, 2, 3];

    for id in tests {
        let session_id = provider
            .provide_session_repository()
            .create_session(
                provider
                    .provide_user_repository()
                    .get_user_by_display_id(id)
                    .await?
                    .unwrap(),
            )
            .await?;

        let response = app
            .borrow_mut()
            .oneshot(
                Request::builder()
                    .method(http::Method::GET)
                    .uri("/users/me")
                    .header("Cookie", format!("session_id={}", session_id))
                    .body(Body::empty())?,
            )
            .await?;
        assert_eq!(response.status(), 200);

        let mut resp_json: Value =
            serde_json::from_slice(&response.into_body().collect().await?.to_bytes())?;

        users_check_by_id(id, &mut resp_json)?;
    }

    Ok(())
}

#[sqlx::test(fixtures("common"))]
async fn get_user_me_unauthorized(pool: sqlx::MySqlPool) -> anyhow::Result<()> {
    let provider = Provider::create_by_pool(pool).await?;
    let di_container = DiContainer::new(provider).await;
    let mut app = make_router(di_container);

    // Test unauthorized case
    let response = app
        .borrow_mut()
        .oneshot(
            Request::builder()
                .method(http::Method::GET)
                .uri("/users/me")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(response.status(), 401);

    Ok(())
}
