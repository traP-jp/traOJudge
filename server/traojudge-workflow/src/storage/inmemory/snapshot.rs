use std::path::Path;

use async_trait::async_trait;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

use super::edge_repository::CREATE_EDGES_TABLE;
use super::job_repository::CREATE_JOBS_TABLE;
use super::pipeline_repository::CREATE_PIPELINES_TABLE;
use super::{InMemoryUnitOfWork, InMemoryUnitOfWorkProvider};
use crate::storage::UnitOfWorkProvider;

/// A private, local SQLite working copy restored from an external checkpoint.
///
/// The caller must serialize writers and durably publish the closed database
/// together with its dispatch outbox before performing any external dispatch.
/// Opening a file alone does not provide distributed durability or fencing.
#[derive(Clone)]
pub struct SnapshotUnitOfWorkProvider(InMemoryUnitOfWorkProvider);

impl SnapshotUnitOfWorkProvider {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(path)
                    .create_if_missing(true)
                    .foreign_keys(true)
                    .journal_mode(SqliteJournalMode::Delete),
            )
            .await?;
        let exists: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'workflow_pipelines'",
        )
        .fetch_one(&pool)
        .await?;
        if exists == 0 {
            let mut transaction = pool.begin().await?;
            for statement in [
                CREATE_PIPELINES_TABLE,
                CREATE_JOBS_TABLE,
                CREATE_EDGES_TABLE,
            ] {
                sqlx::query(statement).execute(&mut *transaction).await?;
            }
            transaction.commit().await?;
        }
        Ok(Self(InMemoryUnitOfWorkProvider { pool }))
    }

    /// Close all connections before reading or publishing the checkpoint file.
    pub async fn close(&self) {
        self.0.pool.close().await;
    }
}

#[async_trait]
impl UnitOfWorkProvider for SnapshotUnitOfWorkProvider {
    type Error = sqlx::Error;
    type UnitOfWork<'provider> = InMemoryUnitOfWork;

    async fn begin(&self) -> Result<Self::UnitOfWork<'_>, Self::Error> {
        self.0.begin().await
    }
}
