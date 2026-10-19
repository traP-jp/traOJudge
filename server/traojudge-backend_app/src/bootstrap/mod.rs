//! 設定の読み込み、依存の組み立て、HTTP・バックグラウンド処理の起動。

use std::net::SocketAddr;

pub async fn serve() -> Result<(), Box<dyn std::error::Error>> {
    let addr = std::env::var("TRAOJUDGE_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3000".to_owned())
        .parse::<SocketAddr>()?;

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "listening");

    axum::serve(listener, crate::http::build_router()).await?;

    Ok(())
}

pub fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "traojudge_backend_app=info".into());

    tracing_subscriber::fmt().with_env_filter(filter).init();
}
