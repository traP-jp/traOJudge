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

pub mod config;
pub mod http;
pub mod models;

pub fn build_router() -> Router {
    Router::new().route("/ping", get(ping))
}

pub async fn run() -> anyhow::Result<()> {
    serve().await
}

pub async fn serve() -> anyhow::Result<()> {
    serve_with_config(config::Config::from_env()?).await
}

pub async fn serve_with_config(config: config::Config) -> anyhow::Result<()> {
    let app = build_router()
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
