use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait UnitOfWork {
    async fn commit(self) -> Result<()>;
}

#[async_trait]
pub trait UnitOfWorkProvider {
    type UnitOfWork: UnitOfWork;

    async fn provide(&self) -> Result<Self::UnitOfWork>;
}
