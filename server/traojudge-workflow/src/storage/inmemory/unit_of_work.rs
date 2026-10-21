use async_trait::async_trait;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::storage::{UnitOfWork, UnitOfWorkProvider};

use super::edge_repository::CREATE_EDGES_TABLE;
use super::job_repository::CREATE_JOBS_TABLE;
use super::pipeline_repository::CREATE_PIPELINES_TABLE;
use super::{InMemoryEdgeRepository, InMemoryJobRepository, InMemoryPipelineRepository};

/// SQLiteをメモリ上で動かすUnit of Work provider。
///
/// `new`ごとに独立したDBを作成し、cloneは同じDBを共有する。
/// 接続は1本で、先行するUnit of Workの終了まで次の`begin`は待機する。
#[derive(Clone)]
pub struct InMemoryUnitOfWorkProvider {
    pool: SqlitePool,
}

impl InMemoryUnitOfWorkProvider {
    pub async fn new() -> Result<Self, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .idle_timeout(None)
            .max_lifetime(None)
            .test_before_acquire(false)
            .connect("sqlite::memory:")
            .await?;

        sqlx::query(CREATE_JOBS_TABLE).execute(&pool).await?;
        sqlx::query(CREATE_EDGES_TABLE).execute(&pool).await?;
        sqlx::query(CREATE_PIPELINES_TABLE).execute(&pool).await?;

        Ok(Self { pool })
    }
}

#[async_trait]
impl UnitOfWorkProvider for InMemoryUnitOfWorkProvider {
    type Error = sqlx::Error;
    type UnitOfWork<'provider> = InMemoryUnitOfWork;

    async fn begin(&self) -> Result<Self::UnitOfWork<'_>, Self::Error> {
        let transaction = self.pool.begin().await?;
        Ok(InMemoryUnitOfWork { transaction })
    }
}

/// 未確定でdropするとSQLxがロールバックするUnit of Work。
pub struct InMemoryUnitOfWork {
    transaction: Transaction<'static, Sqlite>,
}

#[async_trait]
impl UnitOfWork for InMemoryUnitOfWork {
    type Error = sqlx::Error;
    type JobRepository<'work> = InMemoryJobRepository<'work>;
    type EdgeRepository<'work> = InMemoryEdgeRepository<'work>;
    type PipelineRepository<'work> = InMemoryPipelineRepository<'work>;

    fn jobs(&mut self) -> Self::JobRepository<'_> {
        InMemoryJobRepository::new(self.transaction.as_mut())
    }

    fn edges(&mut self) -> Self::EdgeRepository<'_> {
        InMemoryEdgeRepository::new(self.transaction.as_mut())
    }

    fn pipelines(&mut self) -> Self::PipelineRepository<'_> {
        InMemoryPipelineRepository::new(self.transaction.as_mut())
    }

    async fn commit(self) -> Result<(), Self::Error> {
        self.transaction.commit().await
    }

    async fn rollback(self) -> Result<(), Self::Error> {
        self.transaction.rollback().await
    }
}

#[cfg(test)]
mod tests;
