//! PostgreSQL implementation of the workflow storage contracts.
mod edge_repository;
mod job_repository;
mod pipeline_repository;

use async_trait::async_trait;
pub use edge_repository::PostgresEdgeRepository;
pub use job_repository::PostgresJobRepository;
pub use pipeline_repository::PostgresPipelineRepository;
use sqlx::{PgPool, Postgres, Transaction};
use traojudge_workflow::storage::{UnitOfWork, UnitOfWorkProvider};

#[derive(Clone)]
pub struct PostgresUnitOfWorkProvider {
    pool: PgPool,
}
impl PostgresUnitOfWorkProvider {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
pub struct PostgresUnitOfWork {
    transaction: Transaction<'static, Postgres>,
}
#[async_trait]
impl UnitOfWorkProvider for PostgresUnitOfWorkProvider {
    type Error = sqlx::Error;
    type UnitOfWork<'a> = PostgresUnitOfWork;
    async fn begin(&self) -> Result<PostgresUnitOfWork, sqlx::Error> {
        Ok(PostgresUnitOfWork {
            transaction: self.pool.begin().await?,
        })
    }
}
#[async_trait]
impl UnitOfWork for PostgresUnitOfWork {
    type Error = sqlx::Error;
    type JobRepository<'a> = PostgresJobRepository<'a>;
    type EdgeRepository<'a> = PostgresEdgeRepository<'a>;
    type PipelineRepository<'a> = PostgresPipelineRepository<'a>;
    fn jobs(&mut self) -> Self::JobRepository<'_> {
        PostgresJobRepository::new(self.transaction.as_mut())
    }
    fn edges(&mut self) -> Self::EdgeRepository<'_> {
        PostgresEdgeRepository::new(self.transaction.as_mut())
    }
    fn pipelines(&mut self) -> Self::PipelineRepository<'_> {
        PostgresPipelineRepository::new(self.transaction.as_mut())
    }
    async fn commit(self) -> Result<(), sqlx::Error> {
        self.transaction.commit().await
    }
    async fn rollback(self) -> Result<(), sqlx::Error> {
        self.transaction.rollback().await
    }
}
