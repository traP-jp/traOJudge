use self::extractors::{Page, ResourcePair, encode_cursor};
use crate::http::{
    error::ApiError,
    extractors::{
        CurrentSession, IfMatch, OptionalSession, RequestKey, ResourceId, StrictJson, etag,
    },
    middleware::cache,
    schema,
};
use axum::{
    Extension, Json, Router,
    extract::State,
    http::{StatusCode, header},
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use std::sync::Arc;
use traojudge_core::{
    application::contest::ContestService,
    domain::model::{
        account::RoleName,
        contest::{Contest, ContestKind},
        user::UserId,
    },
    ports::repository::contest::ContestUnitOfWorkProvider,
};
mod extractors;
mod mapping;
struct ContestState<U> {
    service: Arc<ContestService<U>>,
    kind: ContestKind,
}

pub(super) fn routes<U: ContestUnitOfWorkProvider + 'static>(service: ContestService<U>) -> Router {
    let service = Arc::new(service);
    Router::new()
        .nest("/contests", router(service.clone(), ContestKind::Regular))
        .nest("/virtual-contests", router(service, ContestKind::Virtual))
}
fn router<U: ContestUnitOfWorkProvider + 'static>(
    service: Arc<ContestService<U>>,
    kind: ContestKind,
) -> Router {
    let mut routes = Router::new()
        .route("/{contestId}", get(get_one::<U>).delete(delete::<U>))
        .route("/{contestId}/problems", get(problems::<U>))
        .route(
            "/{contestId}/problems/{problemId}",
            put(add_problem::<U>).delete(remove_problem::<U>),
        )
        .route("/{contestId}/participation", post(join_individual::<U>))
        .route(
            "/{contestId}/setters/{userId}",
            put(grant::<U>)
                .delete(revoke::<U>)
                .layer(Extension(kind.setter())),
        )
        .route(
            "/{contestId}/participants/{userId}",
            put(grant::<U>)
                .delete(revoke::<U>)
                .layer(Extension(kind.participant())),
        );
    routes = match kind {
        ContestKind::Regular => routes
            .route("/", get(list::<U>).post(create_regular::<U>))
            .route("/{contestId}", axum::routing::patch(update_regular::<U>))
            .route(
                "/{contestId}/admins/{userId}",
                put(grant::<U>)
                    .delete(revoke::<U>)
                    .layer(Extension(RoleName::ContestAdmin)),
            )
            .route("/{contestId}/teams", post(join_team::<U>))
            .route("/{contestId}/teams/{teamId}", get(team::<U>))
            .route(
                "/{contestId}/participation",
                axum::routing::delete(leave_team::<U>),
            ),
        ContestKind::Virtual => routes
            .route("/", get(list::<U>).post(create_virtual::<U>))
            .route("/{contestId}", axum::routing::patch(update_virtual::<U>)),
    };
    routes
        .layer(middleware::map_response(cache::no_store))
        .with_state(Arc::new(ContestState { service, kind }))
}
async fn create_regular<U: ContestUnitOfWorkProvider>(
    State(state): State<Arc<ContestState<U>>>,
    CurrentSession(actor): CurrentSession,
    RequestKey(key): RequestKey,
    StrictJson(input): StrictJson<schema::ContestCreate>,
) -> Result<Response, ApiError> {
    Ok(contest_response(
        StatusCode::CREATED,
        state
            .service
            .create(actor, state.kind, key, input.try_into()?)
            .await?,
    ))
}
async fn create_virtual<U: ContestUnitOfWorkProvider>(
    State(state): State<Arc<ContestState<U>>>,
    CurrentSession(actor): CurrentSession,
    RequestKey(key): RequestKey,
    StrictJson(input): StrictJson<schema::VirtualContestCreate>,
) -> Result<Response, ApiError> {
    Ok(contest_response(
        StatusCode::CREATED,
        state
            .service
            .create(actor, state.kind, key, input.try_into()?)
            .await?,
    ))
}
async fn get_one<U: ContestUnitOfWorkProvider>(
    State(state): State<Arc<ContestState<U>>>,
    OptionalSession(actor): OptionalSession,
    ResourceId(id): ResourceId,
) -> Result<Response, ApiError> {
    Ok(contest_response(
        StatusCode::OK,
        state.service.get(actor, state.kind, id).await?,
    ))
}
async fn list<U: ContestUnitOfWorkProvider>(
    State(state): State<Arc<ContestState<U>>>,
    OptionalSession(actor): OptionalSession,
    page: Page,
) -> Result<Response, ApiError> {
    let page = state
        .service
        .list(actor, state.kind, page.for_kind(state.kind)?)
        .await?;
    let cursor = page.next.map(|cursor| encode_cursor(state.kind, cursor));
    Ok(match state.kind {
        ContestKind::Regular => Json(schema::Contests::new(
            page.items.into_iter().map(Into::into).collect(),
            cursor,
        ))
        .into_response(),
        ContestKind::Virtual => Json(schema::VirtualContests::new(
            page.items.into_iter().map(Into::into).collect(),
            cursor,
        ))
        .into_response(),
    })
}
async fn update_regular<U: ContestUnitOfWorkProvider>(
    State(state): State<Arc<ContestState<U>>>,
    CurrentSession(actor): CurrentSession,
    ResourceId(id): ResourceId,
    IfMatch(revision): IfMatch,
    StrictJson(input): StrictJson<schema::ContestUpdate>,
) -> Result<Response, ApiError> {
    Ok(contest_response(
        StatusCode::OK,
        state
            .service
            .update(actor, state.kind, id, &revision, input.try_into()?)
            .await?,
    ))
}
async fn update_virtual<U: ContestUnitOfWorkProvider>(
    State(state): State<Arc<ContestState<U>>>,
    CurrentSession(actor): CurrentSession,
    ResourceId(id): ResourceId,
    IfMatch(revision): IfMatch,
    StrictJson(input): StrictJson<schema::VirtualContestUpdate>,
) -> Result<Response, ApiError> {
    Ok(contest_response(
        StatusCode::OK,
        state
            .service
            .update(actor, state.kind, id, &revision, input.try_into()?)
            .await?,
    ))
}
async fn delete<U: ContestUnitOfWorkProvider>(
    State(state): State<Arc<ContestState<U>>>,
    CurrentSession(actor): CurrentSession,
    ResourceId(id): ResourceId,
) -> Result<StatusCode, ApiError> {
    state.service.delete(actor, state.kind, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
async fn problems<U: ContestUnitOfWorkProvider>(
    State(state): State<Arc<ContestState<U>>>,
    OptionalSession(actor): OptionalSession,
    ResourceId(id): ResourceId,
) -> Result<Json<schema::ContestProblems>, ApiError> {
    Ok(Json(schema::ContestProblems::new(
        state
            .service
            .problems(actor, state.kind, id)
            .await?
            .into_iter()
            .map(Into::into)
            .collect(),
    )))
}
async fn add_problem<U: ContestUnitOfWorkProvider>(
    State(state): State<Arc<ContestState<U>>>,
    CurrentSession(actor): CurrentSession,
    ResourcePair(id, problem): ResourcePair,
    StrictJson(input): StrictJson<schema::ContestProblemInput>,
) -> Result<Json<schema::ContestProblem>, ApiError> {
    Ok(Json(
        state
            .service
            .add_problem(actor, state.kind, id, problem, input.into())
            .await?
            .into(),
    ))
}
async fn remove_problem<U: ContestUnitOfWorkProvider>(
    State(state): State<Arc<ContestState<U>>>,
    CurrentSession(actor): CurrentSession,
    ResourcePair(id, problem): ResourcePair,
) -> Result<StatusCode, ApiError> {
    state
        .service
        .remove_problem(actor, state.kind, id, problem)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
async fn grant<U: ContestUnitOfWorkProvider>(
    State(state): State<Arc<ContestState<U>>>,
    CurrentSession(actor): CurrentSession,
    ResourcePair(id, user): ResourcePair,
    Extension(role): Extension<RoleName>,
) -> Result<Json<schema::RoleChangeResult>, ApiError> {
    let (assignment, changed) = state
        .service
        .grant_role(actor, state.kind, id, UserId::new(user), role)
        .await?;
    Ok(Json(schema::RoleChangeResult::new(
        assignment.into(),
        changed,
    )))
}
async fn revoke<U: ContestUnitOfWorkProvider>(
    State(state): State<Arc<ContestState<U>>>,
    CurrentSession(actor): CurrentSession,
    ResourcePair(id, user): ResourcePair,
    Extension(role): Extension<RoleName>,
) -> Result<StatusCode, ApiError> {
    state
        .service
        .revoke_role(actor, state.kind, id, UserId::new(user), role)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
async fn join_individual<U: ContestUnitOfWorkProvider>(
    State(state): State<Arc<ContestState<U>>>,
    CurrentSession(actor): CurrentSession,
    ResourceId(id): ResourceId,
    RequestKey(key): RequestKey,
) -> Result<(StatusCode, Json<schema::Participation>), ApiError> {
    Ok((
        StatusCode::CREATED,
        Json(
            state
                .service
                .join_individual(actor, state.kind, id, key)
                .await?
                .into(),
        ),
    ))
}
async fn join_team<U: ContestUnitOfWorkProvider>(
    State(state): State<Arc<ContestState<U>>>,
    CurrentSession(actor): CurrentSession,
    ResourceId(id): ResourceId,
    RequestKey(key): RequestKey,
    StrictJson(input): StrictJson<schema::ContestTeamJoinRequest>,
) -> Result<Response, ApiError> {
    let result = state
        .service
        .join_team(actor, id, key, input.name, input.password)
        .await?;
    let status = if result.created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(schema::ContestTeamParticipation::from(result))).into_response())
}
async fn team<U: ContestUnitOfWorkProvider>(
    State(state): State<Arc<ContestState<U>>>,
    CurrentSession(actor): CurrentSession,
    ResourcePair(id, team): ResourcePair,
) -> Result<Json<schema::ContestTeam>, ApiError> {
    Ok(Json(state.service.team(actor, id, team).await?.into()))
}
async fn leave_team<U: ContestUnitOfWorkProvider>(
    State(state): State<Arc<ContestState<U>>>,
    CurrentSession(actor): CurrentSession,
    ResourceId(id): ResourceId,
) -> Result<StatusCode, ApiError> {
    state.service.leave_team(actor, id).await?;
    Ok(StatusCode::NO_CONTENT)
}
fn contest_response(status: StatusCode, c: Contest) -> Response {
    let headers = [(header::ETAG, etag(&c.revision))];
    match c.kind {
        ContestKind::Regular => (status, headers, Json(schema::Contest::from(c))).into_response(),
        ContestKind::Virtual => {
            (status, headers, Json(schema::VirtualContest::from(c))).into_response()
        }
    }
}
