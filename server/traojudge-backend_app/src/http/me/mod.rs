use crate::http::middleware::cache;
use axum::{Router, middleware, routing::get};
use std::sync::Arc;
use traojudge_core::{
    application::account::AccountService,
    ports::{
        account_services::AccountExternalServices, repository::account::AccountUnitOfWorkProvider,
    },
};
mod developer_tokens;
mod email;
mod profile;
mod sign_in_methods;

pub(super) fn router<
    R: AccountUnitOfWorkProvider + 'static,
    E: AccountExternalServices + 'static,
>(
    service: AccountService<R, E>,
) -> Router {
    Router::new()
        .route(
            "/",
            get(profile::get::<R, E>).patch(profile::update::<R, E>),
        )
        .route("/roles", get(profile::roles::<R, E>))
        .nest("/sign-in-methods", sign_in_methods::router::<R, E>())
        .nest("/developer-tokens", developer_tokens::router::<R, E>())
        .nest("/email-changes", email::router::<R, E>())
        .layer(middleware::map_response(cache::no_store))
        .with_state(Arc::new(service))
}
