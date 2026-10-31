use async_trait::async_trait;
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};

use traojudge_workflow::storage::{UnitOfWork, UnitOfWorkProvider};

use super::{PostgresEdgeRepository, PostgresJobRepository, PostgresPipelineRepository};
use crate::postgres::MIGRATOR;

/// PostgreSQLの接続プールからUnit of Workを開始するprovider。
///
/// `new`は既存のプールを共有し、`connect`は指定したURLから新しいプールを作る。
/// cloneは同じプールを共有する。
#[derive(Clone)]
pub struct PostgresUnitOfWorkProvider {
    pool: PgPool,
}

impl PostgresUnitOfWorkProvider {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;
        Ok(Self::new(pool))
    }

    /// このクレートが持つmigrationを適用する。
    pub async fn migrate(&self) -> Result<(), sqlx::migrate::MigrateError> {
        MIGRATOR.run(&self.pool).await
    }
}

#[async_trait]
impl UnitOfWorkProvider for PostgresUnitOfWorkProvider {
    type Error = sqlx::Error;
    type UnitOfWork<'provider> = PostgresUnitOfWork;

    async fn begin(&self) -> Result<Self::UnitOfWork<'_>, Self::Error> {
        let transaction = self.pool.begin().await?;
        Ok(PostgresUnitOfWork { transaction })
    }
}

/// 未確定でdropするとSQLxがロールバックするUnit of Work。
pub struct PostgresUnitOfWork {
    transaction: Transaction<'static, Postgres>,
}

#[async_trait]
impl UnitOfWork for PostgresUnitOfWork {
    type Error = sqlx::Error;
    type JobRepository<'work> = PostgresJobRepository<'work>;
    type EdgeRepository<'work> = PostgresEdgeRepository<'work>;
    type PipelineRepository<'work> = PostgresPipelineRepository<'work>;

    fn jobs(&mut self) -> Self::JobRepository<'_> {
        PostgresJobRepository::new(self.transaction.as_mut())
    }

    fn edges(&mut self) -> Self::EdgeRepository<'_> {
        PostgresEdgeRepository::new(self.transaction.as_mut())
    }

    fn pipelines(&mut self) -> Self::PipelineRepository<'_> {
        PostgresPipelineRepository::new(self.transaction.as_mut())
    }

    async fn commit(self) -> Result<(), Self::Error> {
        self.transaction.commit().await
    }

    async fn rollback(self) -> Result<(), Self::Error> {
        self.transaction.rollback().await
    }
}
