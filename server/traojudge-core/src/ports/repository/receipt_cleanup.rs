use crate::domain::model::request::{ReceiptCleanupBatchSize, ReceiptCleanupError};
use async_trait::async_trait;
use chrono::{DateTime, Utc};

#[async_trait]
pub trait ReceiptCleanupUnitOfWorkProvider: Send + Sync {
    type UnitOfWork<'a>: ReceiptCleanupUnitOfWork
    where
        Self: 'a;
    async fn begin(&self) -> Result<Self::UnitOfWork<'_>, ReceiptCleanupError>;
}

#[async_trait]
pub trait ReceiptCleanupUnitOfWork: Send + Sized {
    fn now(&self) -> DateTime<Utc>;
    /// cutoff以前に期限切れとなった記録だけを、指定件数まで削除する。
    /// API処理中の本人・記録は待たずに除外し、次回へ回す。
    async fn delete_expired_editorial_receipts(
        &mut self,
        cutoff: DateTime<Utc>,
        batch_size: ReceiptCleanupBatchSize,
    ) -> Result<u64, ReceiptCleanupError>;
    async fn delete_expired_contest_receipts(
        &mut self,
        cutoff: DateTime<Utc>,
        batch_size: ReceiptCleanupBatchSize,
    ) -> Result<u64, ReceiptCleanupError>;
    async fn commit(self) -> Result<(), ReceiptCleanupError>;
    async fn rollback(self) -> Result<(), ReceiptCleanupError>;
}
