use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait UnitOfWork: Send {
    async fn commit(self) -> Result<()>;
    async fn rollback(self) -> Result<()>;
}

#[async_trait]
pub trait UnitOfWorkProvider: Send + Sync {
    type UnitOfWork: UnitOfWork;

    async fn begin(&self) -> Result<Self::UnitOfWork>;
}
