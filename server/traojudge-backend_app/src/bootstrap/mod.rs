//! 設定の読み込み、依存の組み立て、HTTP・バックグラウンド処理の起動。

use std::net::SocketAddr;
use traojudge_infra::postgres::{self, users::PgUserProfileRepository};

pub async fn serve() -> Result<(), Box<dyn std::error::Error>> {
    let addr = std::env::var("TRAOJUDGE_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3000".to_owned())
        .parse::<SocketAddr>()?;

    let database_url = std::env::var("TRAOJUDGE_DATABASE_URL")?;
    let pool = postgres::connect(&database_url).await?;
    postgres::MIGRATOR.run(&pool).await?;
    let router = crate::http::build_router(PgUserProfileRepository::new(pool));

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "listening");

    axum::serve(listener, router).await?;

    Ok(())
}

pub fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "traojudge_backend_app=info".into());

    tracing_subscriber::fmt().with_env_filter(filter).init();
}
