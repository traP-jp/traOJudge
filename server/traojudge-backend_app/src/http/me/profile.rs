use crate::http::{
    error::ApiError,
    extractors::{CurrentSession, IfMatch, StrictJson, etag},
    schema,
};
use axum::{
    Json,
    extract::State,
    http::header,
    response::{IntoResponse, Response},
};
use std::sync::Arc;
use traojudge_core::{
    application::account::AccountService,
    domain::model::account::*,
    ports::{
        account_services::AccountExternalServices, repository::account::AccountUnitOfWorkProvider,
    },
};

pub(super) async fn get<R: AccountUnitOfWorkProvider, E: AccountExternalServices>(
    State(service): State<Arc<AccountService<R, E>>>,
    CurrentSession(actor): CurrentSession,
) -> Result<Response, ApiError> {
    Ok(current_response(service.current_account(actor).await?))
}
pub(super) async fn update<R: AccountUnitOfWorkProvider, E: AccountExternalServices>(
    State(service): State<Arc<AccountService<R, E>>>,
    CurrentSession(actor): CurrentSession,
    IfMatch(revision): IfMatch,
    StrictJson(input): StrictJson<schema::ProfileUpdate>,
) -> Result<Response, ApiError> {
    Ok(current_response(
        service
            .update_profile(actor, &revision, input.into())
            .await?,
    ))
}
pub(super) async fn roles<R: AccountUnitOfWorkProvider, E: AccountExternalServices>(
    State(service): State<Arc<AccountService<R, E>>>,
    CurrentSession(actor): CurrentSession,
) -> Result<Json<schema::RoleAssignments>, ApiError> {
    let roles = service.current_account(actor).await?.roles;
    Ok(Json(schema::RoleAssignments::new(
        roles.into_iter().map(Into::into).collect(),
    )))
}
fn current_response(account: CurrentAccount) -> Response {
    (
        [(header::ETAG, etag(&account.profile.revision))],
        Json(schema::CurrentUser::from(account)),
    )
        .into_response()
}
