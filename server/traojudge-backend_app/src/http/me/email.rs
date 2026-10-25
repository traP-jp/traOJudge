use crate::http::{
    error::ApiError,
    extractors::{CurrentSession, StrictJson},
    schema,
};
use axum::{Json, Router, extract::State, http::StatusCode, routing::post};
use std::sync::Arc;
use traojudge_core::{
    application::account::AccountService,
    ports::{
        account_services::AccountExternalServices, repository::account::AccountUnitOfWorkProvider,
    },
};

use chrono::Utc;
use uuid::Uuid;
pub(super) fn router<
    R: AccountUnitOfWorkProvider + 'static,
    E: AccountExternalServices + 'static,
>() -> Router<Arc<AccountService<R, E>>> {
    Router::new()
        .route("/", post(request::<R, E>))
        .route("/complete", post(complete::<R, E>))
}
async fn request<R: AccountUnitOfWorkProvider, E: AccountExternalServices>(
    State(service): State<Arc<AccountService<R, E>>>,
    CurrentSession(actor): CurrentSession,
    StrictJson(input): StrictJson<schema::EmailChangeRequest>,
) -> Result<(StatusCode, Json<schema::Accepted>), ApiError> {
    service.request_email_change(actor, &input.email).await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(schema::Accepted::new(
            Uuid::new_v4(),
            Utc::now().to_rfc3339(),
        )),
    ))
}
async fn complete<R: AccountUnitOfWorkProvider, E: AccountExternalServices>(
    State(service): State<Arc<AccountService<R, E>>>,
    CurrentSession(actor): CurrentSession,
    StrictJson(input): StrictJson<schema::TokenConfirmation>,
) -> Result<StatusCode, ApiError> {
    service.complete_email_change(actor, &input.token).await?;
    Ok(StatusCode::NO_CONTENT)
}
