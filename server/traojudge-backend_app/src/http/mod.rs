use axum::{Router, routing::get};
use std::sync::Arc;
use traojudge_core::{
    application::users::GetUserProfile, ports::repository::user::UserProfileRepository,
};

pub mod schema;
mod users;

pub fn build_router<R: UserProfileRepository + 'static>(repository: R) -> Router {
    Router::new()
        .route("/ping", get(ping))
        .route("/api/v1/users/{userId}", get(users::get_user_profile::<R>))
        .with_state(Arc::new(GetUserProfile::new(repository)))
}

async fn ping() -> &'static str {
    "pong"
}
