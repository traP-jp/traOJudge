use crate::http::{
    error::ApiError,
    extractors::{CurrentSession, ResourceId, StrictJson},
    schema,
};
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{delete, get},
};
use std::sync::Arc;
use traojudge_core::{
    application::account::AccountService,
    domain::model::account::*,
    ports::{
        account_services::AccountExternalServices, repository::account::AccountUnitOfWorkProvider,
    },
};

use chrono::{DateTime, Utc};
pub(super) fn router<
    R: AccountUnitOfWorkProvider + 'static,
    E: AccountExternalServices + 'static,
>() -> Router<Arc<AccountService<R, E>>> {
    Router::new()
        .route("/", get(list::<R, E>).post(issue::<R, E>))
        .route("/{id}", delete(revoke::<R, E>))
}
async fn list<R: AccountUnitOfWorkProvider, E: AccountExternalServices>(
    State(service): State<Arc<AccountService<R, E>>>,
    CurrentSession(actor): CurrentSession,
) -> Result<Json<schema::DeveloperTokens>, ApiError> {
    Ok(Json(schema::DeveloperTokens::new(
        service
            .developer_tokens(actor)
            .await?
            .into_iter()
            .map(Into::into)
            .collect(),
    )))
}
async fn issue<R: AccountUnitOfWorkProvider, E: AccountExternalServices>(
    State(service): State<Arc<AccountService<R, E>>>,
    CurrentSession(actor): CurrentSession,
    StrictJson(input): StrictJson<schema::DeveloperTokenRequest>,
) -> Result<(StatusCode, Json<schema::IssuedDeveloperToken>), ApiError> {
    let expires_at = input
        .expires_at
        .map(|value| DateTime::parse_from_rfc3339(&value).map(|time| time.with_timezone(&Utc)))
        .transpose()
        .map_err(|_| AccountError::validation("expiresAt", "must be an RFC3339 timestamp"))?;
    let (token, secret) = service
        .issue_developer_token(actor, input.name, expires_at)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(schema::IssuedDeveloperToken::new(
            token.into(),
            secret.expose().to_owned(),
        )),
    ))
}
async fn revoke<R: AccountUnitOfWorkProvider, E: AccountExternalServices>(
    State(service): State<Arc<AccountService<R, E>>>,
    CurrentSession(actor): CurrentSession,
    ResourceId(id): ResourceId,
) -> Result<StatusCode, ApiError> {
    service.revoke_developer_token(actor, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
