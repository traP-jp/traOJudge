use axum::{Router, middleware as axum_middleware};
use std::sync::Arc;
use traojudge_core::{
    application::{
        account::AccountService, contest::ContestService, editorial::EditorialService,
        session::SessionService, users::GetUserProfile,
    },
    ports::{
        account_services::AccountExternalServices,
        repository::{
            account::{AccountUnitOfWorkProvider, SessionRepository},
            contest::ContestUnitOfWorkProvider,
            editorial::EditorialUnitOfWorkProvider,
            user::UserProfileRepository,
        },
    },
};
mod contests;
mod editorials;
mod error;
mod extractors;
mod mapping;
mod me;
pub mod middleware;
pub mod schema;
pub mod users;
use middleware::{
    origin::{self, WebOrigin},
    session,
};

pub fn build_router<P, R, E, S, U, C>(
    profiles: GetUserProfile<P>,
    accounts: AccountService<R, E>,
    sessions: SessionService<S>,
    editorials: EditorialService<U>,
    contests: ContestService<C>,
    origin: WebOrigin,
) -> Router
where
    P: UserProfileRepository + 'static,
    R: AccountUnitOfWorkProvider + 'static,
    E: AccountExternalServices + 'static,
    S: SessionRepository + 'static,
    U: EditorialUnitOfWorkProvider + 'static,
    C: ContestUnitOfWorkProvider + 'static,
{
    let api = Router::new()
        .nest("/users", users::router(profiles))
        .nest("/me", me::router(accounts))
        .merge(editorials::routes(editorials))
        .merge(contests::routes(contests))
        .layer(axum_middleware::from_fn_with_state(
            Arc::new(sessions),
            session::authenticate::<S>,
        ))
        .layer(axum_middleware::from_fn_with_state(
            origin,
            origin::verify_origin,
        ));
    Router::new().nest("/api/v1", api)
}
