use axum::{
    Router,
    http::{HeaderValue, Method},
    routing::get,
};
use tower_http::{
    LatencyUnit,
    classify::{ServerErrorsAsFailures, SharedClassifier},
    cors::CorsLayer,
    trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer},
};
use tracing::Level;

pub mod app_state;
pub mod config;
pub mod http;
pub mod models;

pub use app_state::AppState;

pub fn build_router<P>(state: AppState<P>) -> Router
where
    P: Clone + Send + Sync + 'static,
{
    Router::new().route("/ping", get(ping)).with_state(state)
}

pub async fn run() -> anyhow::Result<()> {
    serve().await
}

pub async fn serve() -> anyhow::Result<()> {
    serve_with_config(config::Config::from_env()?).await
}

pub async fn serve_with_config(config: config::Config) -> anyhow::Result<()> {
    let state = AppState::from_config(&config).await?;
    let app = build_router(state)
        .layer(trace_layer())
        .layer(cors_layer(&config)?);

    let listener = tokio::net::TcpListener::bind(config.addr).await?;
    tracing::info!(addr = %listener.local_addr()?, "listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn ping() -> &'static str {
    "pong"
}

type HttpTraceLayer = TraceLayer<SharedClassifier<ServerErrorsAsFailures>>;

fn trace_layer() -> HttpTraceLayer {
    TraceLayer::new_for_http()
        .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
        .on_request(DefaultOnRequest::new().level(Level::INFO))
        .on_response(
            DefaultOnResponse::new()
                .level(Level::INFO)
                .latency_unit(LatencyUnit::Millis),
        )
}

fn cors_layer(config: &config::Config) -> anyhow::Result<CorsLayer> {
    let frontend_origins = config
        .frontend_origins
        .iter()
        .map(|origin| origin.parse::<HeaderValue>())
        .collect::<Result<Vec<_>, _>>()?;

    Ok(CorsLayer::new()
        .allow_origin(frontend_origins)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers(tower_http::cors::Any))
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("SIGINT received, starting graceful shutdown");
        }
        _ = terminate => {
            tracing::info!("SIGTERM received, starting graceful shutdown");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct FakeProvider;

    #[test]
    fn build_router_accepts_non_database_state() {
        let _ = build_router(AppState::new(FakeProvider));
    }
}
