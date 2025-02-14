use di::DiContainer;
use tower_http::trace::TraceLayer;

pub mod di;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
pub mod usecase;

pub async fn run() -> anyhow::Result<()> {
    let di_container = DiContainer::new().await;

    let app = presentation::handler::make_router(di_container).layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;

    tracing::debug!("listening on {}", listener.local_addr()?);

    axum::serve(listener, app).await?;

    Ok(())
}