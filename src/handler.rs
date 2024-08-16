use axum::{routing::get, Router};

use crate::repository::Repository;

async fn pong() -> &'static str {
    "pong"
}

pub fn make_router(app_state: Repository) -> Router {
    let root_router = Router::new().route("/ping", get(pong));

    Router::new().nest("/", root_router).with_state(app_state)
}
