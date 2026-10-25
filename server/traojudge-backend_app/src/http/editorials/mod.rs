use self::extractors::{AuthorIds, CreationKey, Page, encode_cursor};
use crate::http::{
    error::ApiError,
    extractors::{CurrentSession, IfMatch, OptionalSession, ResourceId, StrictJson, etag},
    middleware::cache,
    schema,
};
use axum::{
    Json, Router,
    extract::State,
    http::{StatusCode, header},
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use std::sync::Arc;
use traojudge_core::{
    application::editorial::EditorialService,
    domain::model::{editorial::Editorial, user::UserId},
    ports::repository::editorial::EditorialUnitOfWorkProvider,
};
mod extractors;

pub(super) fn routes<U: EditorialUnitOfWorkProvider + 'static>(
    service: EditorialService<U>,
) -> Router {
    let editorials = Router::new()
        .route("/", post(create::<U>))
        .route(
            "/{editorialId}",
            get(get_one::<U>).patch(update::<U>).delete(delete::<U>),
        )
        .route(
            "/{editorialId}/publication",
            put(publish::<U>).delete(unpublish::<U>),
        )
        .route(
            "/{editorialId}/authors/{userId}",
            put(grant_author::<U>).delete(revoke_author::<U>),
        );
    Router::new()
        .nest("/editorials", editorials)
        .route("/problems/{problemId}/editorials", get(list::<U>))
        .layer(middleware::map_response(cache::no_store))
        .with_state(Arc::new(service))
}
async fn create<U: EditorialUnitOfWorkProvider>(
    State(service): State<Arc<EditorialService<U>>>,
    CurrentSession(actor): CurrentSession,
    CreationKey(key): CreationKey,
    StrictJson(input): StrictJson<schema::EditorialCreate>,
) -> Result<Response, ApiError> {
    Ok(editorial_response(
        StatusCode::CREATED,
        service.create(actor, key, input.into()).await?,
    ))
}
async fn get_one<U: EditorialUnitOfWorkProvider>(
    State(service): State<Arc<EditorialService<U>>>,
    OptionalSession(actor): OptionalSession,
    ResourceId(id): ResourceId,
) -> Result<Response, ApiError> {
    Ok(editorial_response(
        StatusCode::OK,
        service.get(actor, id).await?,
    ))
}
async fn list<U: EditorialUnitOfWorkProvider>(
    State(service): State<Arc<EditorialService<U>>>,
    OptionalSession(actor): OptionalSession,
    ResourceId(problem_id): ResourceId,
    Page(page): Page,
) -> Result<Json<schema::Editorials>, ApiError> {
    let page = service.list(actor, problem_id, page).await?;
    Ok(Json(schema::Editorials::new(
        page.items.into_iter().map(Into::into).collect(),
        page.next_cursor.map(encode_cursor),
    )))
}
async fn update<U: EditorialUnitOfWorkProvider>(
    State(service): State<Arc<EditorialService<U>>>,
    CurrentSession(actor): CurrentSession,
    ResourceId(id): ResourceId,
    IfMatch(revision): IfMatch,
    StrictJson(input): StrictJson<schema::EditorialUpdate>,
) -> Result<Response, ApiError> {
    Ok(editorial_response(
        StatusCode::OK,
        service.update(actor, id, &revision, input.into()).await?,
    ))
}
async fn grant_author<U: EditorialUnitOfWorkProvider>(
    State(service): State<Arc<EditorialService<U>>>,
    CurrentSession(actor): CurrentSession,
    AuthorIds(id, user_id): AuthorIds,
) -> Result<Json<schema::RoleChangeResult>, ApiError> {
    let (assignment, changed) = service
        .grant_author(actor, id, UserId::new(user_id))
        .await?;
    Ok(Json(schema::RoleChangeResult::new(
        assignment.into(),
        changed,
    )))
}
async fn revoke_author<U: EditorialUnitOfWorkProvider>(
    State(service): State<Arc<EditorialService<U>>>,
    CurrentSession(actor): CurrentSession,
    AuthorIds(id, user_id): AuthorIds,
) -> Result<StatusCode, ApiError> {
    service
        .revoke_author(actor, id, UserId::new(user_id))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
async fn delete<U: EditorialUnitOfWorkProvider>(
    State(service): State<Arc<EditorialService<U>>>,
    CurrentSession(actor): CurrentSession,
    ResourceId(id): ResourceId,
) -> Result<StatusCode, ApiError> {
    service.delete(actor, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
async fn publish<U: EditorialUnitOfWorkProvider>(
    State(service): State<Arc<EditorialService<U>>>,
    CurrentSession(actor): CurrentSession,
    ResourceId(id): ResourceId,
    IfMatch(revision): IfMatch,
) -> Result<Response, ApiError> {
    let editorial = service.publish(actor, id, &revision).await?;
    let tag = etag(&editorial.revision);
    Ok((
        [(header::ETAG, tag)],
        Json(schema::Visibility::from(editorial)),
    )
        .into_response())
}
async fn unpublish<U: EditorialUnitOfWorkProvider>(
    State(service): State<Arc<EditorialService<U>>>,
    CurrentSession(actor): CurrentSession,
    ResourceId(id): ResourceId,
) -> Result<StatusCode, ApiError> {
    service.unpublish(actor, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
fn editorial_response(status: StatusCode, editorial: Editorial) -> Response {
    (
        status,
        [(header::ETAG, etag(&editorial.revision))],
        Json(schema::Editorial::from(editorial)),
    )
        .into_response()
}
