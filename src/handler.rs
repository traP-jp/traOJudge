use axum::{routing::get, Router};

use crate::repository::Repository;

mod users;

pub fn make_router(app_state: Repository) -> Router {
    let users_router = Router::new().route("/me", get(users::get_me));

    Router::new()
        .nest("/users", users_router)
        .with_state(app_state)
}
