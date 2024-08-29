use axum::{
    routing::{get, post},
    Router,
};

use crate::repository::Repository;

mod authentication;
mod users;

pub fn make_router(app_state: Repository) -> Router {
    let authentication_router =
        Router::new().route("/signup/request", post(authentication::sign_up_request));

    let users_router = Router::new().route("/me", get(users::get_me));

    Router::new()
        .nest("/", authentication_router)
        .nest("/users", users_router)
        .with_state(app_state)
}
