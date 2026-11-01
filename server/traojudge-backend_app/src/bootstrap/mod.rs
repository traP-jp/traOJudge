//! 設定の読み込み、依存の組み立て、HTTP・バックグラウンド処理の起動。

use crate::http::{build_router, middleware::origin::WebOrigin};
use std::net::SocketAddr;
use tokio::task::JoinSet;
use traojudge_core::application::{
    account::AccountService, contest::ContestService, editorial::EditorialService,
    receipt_cleanup::ReceiptCleanupService, session::SessionService, users::GetUserProfile,
};
use traojudge_infra::{
    account_services::UnconfiguredAccountServices,
    postgres::{
        self, accounts::PgAccountUnitOfWorkProvider, contest::PgContestUnitOfWorkProvider,
        editorial::PgEditorialUnitOfWorkProvider,
        receipt_cleanup::PgReceiptCleanupUnitOfWorkProvider, sessions::PgSessionRepository,
        users::PgUserProfileRepository,
    },
};

mod receipt_cleanup;

pub async fn serve() -> Result<(), Box<dyn std::error::Error>> {
    let addr = std::env::var("TRAOJUDGE_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3000".to_owned())
        .parse::<SocketAddr>()?;

    let origin = WebOrigin::parse(&std::env::var("TRAOJUDGE_WEB_ORIGIN")?)?;
    let database_url = std::env::var("TRAOJUDGE_DATABASE_URL")?;
    let cleanup_config = receipt_cleanup::Config::from_env()?;
    let pool = postgres::connect(&database_url).await?;
    postgres::MIGRATOR.run(&pool).await?;
    let receipt_retention = std::env::var("TRAOJUDGE_EDITORIAL_IDEMPOTENCY_TTL_SECONDS")
        .unwrap_or_else(|_| "86400".to_owned())
        .parse::<u32>()?;
    let editorials = EditorialService::new(
        PgEditorialUnitOfWorkProvider::new(pool.clone()),
        chrono::Duration::seconds(i64::from(receipt_retention)),
    )?;
    let contest_retention = std::env::var("TRAOJUDGE_CONTEST_IDEMPOTENCY_TTL_SECONDS")
        .unwrap_or_else(|_| "86400".to_owned())
        .parse::<u32>()?;
    let contests = ContestService::new(
        PgContestUnitOfWorkProvider::new(pool.clone()),
        chrono::Duration::seconds(i64::from(contest_retention)),
    )?;
    let cleanup_service = ReceiptCleanupService::new(
        PgReceiptCleanupUnitOfWorkProvider::new(pool.clone()),
        cleanup_config.batch_size,
    );
    let router = build_router(
        GetUserProfile::new(PgUserProfileRepository::new(pool.clone())),
        AccountService::new(
            PgAccountUnitOfWorkProvider::new(pool.clone()),
            UnconfiguredAccountServices,
        ),
        SessionService::new(PgSessionRepository::new(pool)),
        editorials,
        contests,
        origin,
    );

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "listening");

    // JoinSetの破棄でも中止するため、serveの失敗・キャンセル時にworkerを残さない。
    let mut background_tasks = JoinSet::new();
    background_tasks.spawn(receipt_cleanup::run(
        cleanup_service,
        cleanup_config.interval,
    ));
    axum::serve(listener, router).await?;
    background_tasks.shutdown().await;

    Ok(())
}

pub fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "traojudge_backend_app=info".into());

    tracing_subscriber::fmt().with_env_filter(filter).init();
}
