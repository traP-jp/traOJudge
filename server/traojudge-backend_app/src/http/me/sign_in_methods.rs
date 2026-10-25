use crate::http::{
    error::ApiError,
    extractors::{CurrentSession, ResourceId, StrictJson},
    schema,
};
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{delete, get, post},
};
use std::sync::Arc;
use traojudge_core::{
    application::account::AccountService,
    ports::{
        account_services::AccountExternalServices, repository::account::AccountUnitOfWorkProvider,
    },
};

pub(super) fn router<
    R: AccountUnitOfWorkProvider + 'static,
    E: AccountExternalServices + 'static,
>() -> Router<Arc<AccountService<R, E>>> {
    Router::new()
        .route("/", get(list::<R, E>))
        .route("/{id}", delete(remove::<R, E>))
        .nest(
            "/external-authorizations",
            Router::new()
                .route("/", post(start::<R, E>))
                .route("/{id}/complete", post(complete::<R, E>)),
        )
}
async fn list<R: AccountUnitOfWorkProvider, E: AccountExternalServices>(
    State(service): State<Arc<AccountService<R, E>>>,
    CurrentSession(actor): CurrentSession,
) -> Result<Json<schema::SignInMethods>, ApiError> {
    Ok(Json(schema::SignInMethods::new(
        service
            .sign_in_methods(actor)
            .await?
            .into_iter()
            .map(Into::into)
            .collect(),
    )))
}
async fn remove<R: AccountUnitOfWorkProvider, E: AccountExternalServices>(
    State(service): State<Arc<AccountService<R, E>>>,
    CurrentSession(actor): CurrentSession,
    ResourceId(id): ResourceId,
) -> Result<StatusCode, ApiError> {
    service.remove_sign_in_method(actor, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
async fn start<R: AccountUnitOfWorkProvider, E: AccountExternalServices>(
    State(service): State<Arc<AccountService<R, E>>>,
    CurrentSession(actor): CurrentSession,
    StrictJson(input): StrictJson<schema::ExternalAuthorizationRequest>,
) -> Result<(StatusCode, Json<schema::Authorization>), ApiError> {
    let authorization = service
        .start_authorization(actor, input.provider.into())
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(schema::Authorization::new(
            authorization.id,
            authorization.url,
            authorization.expires_at.to_rfc3339(),
        )),
    ))
}
async fn complete<R: AccountUnitOfWorkProvider, E: AccountExternalServices>(
    State(service): State<Arc<AccountService<R, E>>>,
    CurrentSession(actor): CurrentSession,
    ResourceId(id): ResourceId,
    StrictJson(input): StrictJson<schema::AuthorizationCompletion>,
) -> Result<(StatusCode, Json<schema::SignInMethod>), ApiError> {
    let linked = service
        .complete_authorization(actor, id, &input.code, &input.state)
        .await?;
    Ok((
        if linked.created {
            StatusCode::CREATED
        } else {
            StatusCode::OK
        },
        Json(linked.method.into()),
    ))
}
