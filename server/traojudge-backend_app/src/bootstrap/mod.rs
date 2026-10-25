//! 設定の読み込み、依存の組み立て、HTTP・バックグラウンド処理の起動。

use crate::http::{build_router, middleware::origin::WebOrigin};
use std::net::SocketAddr;
use traojudge_core::application::{
    account::AccountService, editorial::EditorialService, session::SessionService,
    users::GetUserProfile,
};
use traojudge_infra::{
    account_services::UnconfiguredAccountServices,
    postgres::{
        self, accounts::PgAccountUnitOfWorkProvider, editorial::PgEditorialUnitOfWorkProvider,
        sessions::PgSessionRepository, users::PgUserProfileRepository,
    },
};

pub async fn serve() -> Result<(), Box<dyn std::error::Error>> {
    let addr = std::env::var("TRAOJUDGE_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3000".to_owned())
        .parse::<SocketAddr>()?;

    let origin = WebOrigin::parse(&std::env::var("TRAOJUDGE_WEB_ORIGIN")?)?;
    let database_url = std::env::var("TRAOJUDGE_DATABASE_URL")?;
    let pool = postgres::connect(&database_url).await?;
    postgres::MIGRATOR.run(&pool).await?;
    let receipt_retention = std::env::var("TRAOJUDGE_EDITORIAL_IDEMPOTENCY_TTL_SECONDS")
        .unwrap_or_else(|_| "86400".to_owned())
        .parse::<u32>()?;
    let editorials = EditorialService::new(
        PgEditorialUnitOfWorkProvider::new(pool.clone()),
        chrono::Duration::seconds(i64::from(receipt_retention)),
    )?;
    let router = build_router(
        GetUserProfile::new(PgUserProfileRepository::new(pool.clone())),
        AccountService::new(
            PgAccountUnitOfWorkProvider::new(pool.clone()),
            UnconfiguredAccountServices,
        ),
        SessionService::new(PgSessionRepository::new(pool)),
        editorials,
        origin,
    );

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
