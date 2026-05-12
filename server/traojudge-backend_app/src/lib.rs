use std::net::SocketAddr;

use axum::{Router, routing::get};

pub fn build_router() -> Router {
    Router::new().route("/ping", get(ping))
}

pub async fn serve() -> Result<(), Box<dyn std::error::Error>> {
    let addr = std::env::var("TRAOJUDGE_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3000".to_owned())
        .parse::<SocketAddr>()?;

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "listening");

    axum::serve(listener, build_router()).await?;

    Ok(())
}

async fn ping() -> &'static str {
    "pong"
}
