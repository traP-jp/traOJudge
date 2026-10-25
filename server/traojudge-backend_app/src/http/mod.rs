use axum::{Router, middleware as axum_middleware};
use std::sync::Arc;
use traojudge_core::{
    application::{
        account::AccountService, editorial::EditorialService, session::SessionService,
        users::GetUserProfile,
    },
    ports::{
        account_services::AccountExternalServices,
        repository::{
            account::{AccountUnitOfWorkProvider, SessionRepository},
            editorial::EditorialUnitOfWorkProvider,
            user::UserProfileRepository,
        },
    },
};
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

pub fn build_router<P, R, E, S, U>(
    profiles: GetUserProfile<P>,
    accounts: AccountService<R, E>,
    sessions: SessionService<S>,
    editorials: EditorialService<U>,
    origin: WebOrigin,
) -> Router
where
    P: UserProfileRepository + 'static,
    R: AccountUnitOfWorkProvider + 'static,
    E: AccountExternalServices + 'static,
    S: SessionRepository + 'static,
    U: EditorialUnitOfWorkProvider + 'static,
{
    let api = Router::new()
        .nest("/users", users::router(profiles))
        .nest("/me", me::router(accounts))
        .merge(editorials::routes(editorials))
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
