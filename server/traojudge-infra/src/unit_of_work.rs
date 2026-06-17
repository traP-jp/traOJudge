use async_trait::async_trait;
use sqlx::{MySql, Transaction};
use traojudge_core::domain::service::unit_of_work_provider::{UnitOfWork, UnitOfWorkProvider};

use crate::database::{DatabaseConnection, MariaDbRepositoryProvider};

pub type MariaDbTransaction = Transaction<'static, MySql>;

#[derive(Clone, Debug)]
pub struct MariaDbUnitOfWorkProvider {
    database: DatabaseConnection,
}

impl MariaDbUnitOfWorkProvider {
    pub fn new(database: DatabaseConnection) -> Self {
        Self { database }
    }

    pub fn database(&self) -> &DatabaseConnection {
        &self.database
    }

    pub fn provide_repository_provider(&self) -> MariaDbRepositoryProvider {
        MariaDbRepositoryProvider::new(self.database.clone())
    }
}

#[derive(Debug)]
pub struct MariaDbUnitOfWork {
    transaction: MariaDbTransaction,
}

impl MariaDbUnitOfWork {
    pub fn transaction(&mut self) -> &mut MariaDbTransaction {
        &mut self.transaction
    }
}

#[async_trait]
impl UnitOfWorkProvider for MariaDbUnitOfWorkProvider {
    type UnitOfWork = MariaDbUnitOfWork;

    async fn begin(&self) -> anyhow::Result<Self::UnitOfWork> {
        let transaction = self.database.pool().begin().await?;
        Ok(MariaDbUnitOfWork { transaction })
    }
}

#[async_trait]
impl UnitOfWork for MariaDbUnitOfWork {
    async fn commit(self) -> anyhow::Result<()> {
        self.transaction.commit().await?;
        Ok(())
    }

    async fn rollback(self) -> anyhow::Result<()> {
        self.transaction.rollback().await?;
        Ok(())
    }
}
