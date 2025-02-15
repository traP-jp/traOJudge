use di::DiContainer;
use infrastructure::provider::Provider;
use tower_http::trace::TraceLayer;

pub mod di;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
pub mod usecase;

pub use presentation::handler::make_router;

pub async fn run() -> anyhow::Result<()> {
    let provider = Provider::new().await.unwrap();
    let di_container = DiContainer::new(provider).await;

    let app = make_router(di_container).layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;

    tracing::debug!("listening on {}", listener.local_addr()?);

    axum::serve(listener, app).await?;

    Ok(())
}
