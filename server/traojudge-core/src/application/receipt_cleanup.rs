use crate::{
    domain::model::request::{ReceiptCleanupBatchSize, ReceiptCleanupCounts, ReceiptCleanupError},
    ports::repository::receipt_cleanup::{
        ReceiptCleanupUnitOfWork, ReceiptCleanupUnitOfWorkProvider,
    },
};

pub struct ReceiptCleanupService<U> {
    unit_of_work: U,
    batch_size: ReceiptCleanupBatchSize,
}
impl<U: ReceiptCleanupUnitOfWorkProvider> ReceiptCleanupService<U> {
    pub fn new(unit_of_work: U, batch_size: ReceiptCleanupBatchSize) -> Self {
        Self {
            unit_of_work,
            batch_size,
        }
    }

    /// 1回の実行を有界なUoWにし、両方の再送記録を同じcutoffで削除する。
    pub async fn run_batch(&self) -> Result<ReceiptCleanupCounts, ReceiptCleanupError> {
        let mut tx = self.unit_of_work.begin().await?;
        let cutoff = tx.now();
        let editorial_receipts = tx
            .delete_expired_editorial_receipts(cutoff, self.batch_size)
            .await?;
        let contest_receipts = tx
            .delete_expired_contest_receipts(cutoff, self.batch_size)
            .await?;
        tx.commit().await?;
        Ok(ReceiptCleanupCounts {
            editorial_receipts,
            contest_receipts,
        })
    }
}
