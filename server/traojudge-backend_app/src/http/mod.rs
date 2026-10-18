use axum::{Router, routing::get};

pub mod schema;

pub fn build_router() -> Router {
    Router::new().route("/ping", get(ping))
}

async fn ping() -> &'static str {
    "pong"
}
