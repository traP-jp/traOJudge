use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use traojudge_core::{
    domain::model::request::{ReceiptCleanupBatchSize, ReceiptCleanupError},
    ports::repository::receipt_cleanup::{
        ReceiptCleanupUnitOfWork, ReceiptCleanupUnitOfWorkProvider,
    },
};

pub struct PgReceiptCleanupUnitOfWorkProvider {
    pool: PgPool,
}
impl PgReceiptCleanupUnitOfWorkProvider {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
pub struct PgReceiptCleanupUnitOfWork {
    tx: Transaction<'static, Postgres>,
    now: DateTime<Utc>,
}

#[async_trait]
impl ReceiptCleanupUnitOfWorkProvider for PgReceiptCleanupUnitOfWorkProvider {
    type UnitOfWork<'a> = PgReceiptCleanupUnitOfWork;
    async fn begin(&self) -> Result<Self::UnitOfWork<'_>, ReceiptCleanupError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(ReceiptCleanupError::storage)?;
        let now = sqlx::query_scalar("SELECT clock_timestamp()")
            .fetch_one(&mut *tx)
            .await
            .map_err(ReceiptCleanupError::storage)?;
        Ok(PgReceiptCleanupUnitOfWork { tx, now })
    }
}

#[async_trait]
impl ReceiptCleanupUnitOfWork for PgReceiptCleanupUnitOfWork {
    fn now(&self) -> DateTime<Utc> {
        self.now
    }
    async fn delete_expired_editorial_receipts(
        &mut self,
        cutoff: DateTime<Utc>,
        batch_size: ReceiptCleanupBatchSize,
    ) -> Result<u64, ReceiptCleanupError> {
        // 本人の行も確保し、API側がUoW開始時に検査した期限内の記録を処理途中で失わせない。
        let result = sqlx::query(
            "WITH expired AS MATERIALIZED (
                SELECT r.user_id, r.key
                FROM editorial_creation_receipts r JOIN users u ON u.id = r.user_id
                WHERE r.expires_at <= $1
                ORDER BY r.expires_at, r.user_id, r.key
                LIMIT $2 FOR UPDATE OF u, r SKIP LOCKED
            )
            DELETE FROM editorial_creation_receipts r USING expired
            WHERE r.user_id = expired.user_id AND r.key = expired.key AND r.expires_at <= $1",
        )
        .bind(cutoff)
        .bind(i64::from(batch_size.get()))
        .execute(&mut *self.tx)
        .await
        .map_err(ReceiptCleanupError::storage)?;
        Ok(result.rows_affected())
    }
    async fn delete_expired_contest_receipts(
        &mut self,
        cutoff: DateTime<Utc>,
        batch_size: ReceiptCleanupBatchSize,
    ) -> Result<u64, ReceiptCleanupError> {
        let result = sqlx::query(
            "WITH expired AS MATERIALIZED (
                SELECT r.user_id, r.operation, r.scope_id, r.key
                FROM contest_request_receipts r JOIN users u ON u.id = r.user_id
                WHERE r.expires_at <= $1
                ORDER BY r.expires_at, r.user_id, r.operation, r.scope_id, r.key
                LIMIT $2 FOR UPDATE OF u, r SKIP LOCKED
            )
            DELETE FROM contest_request_receipts r USING expired
            WHERE r.user_id = expired.user_id AND r.operation = expired.operation
                AND r.scope_id = expired.scope_id AND r.key = expired.key AND r.expires_at <= $1",
        )
        .bind(cutoff)
        .bind(i64::from(batch_size.get()))
        .execute(&mut *self.tx)
        .await
        .map_err(ReceiptCleanupError::storage)?;
        Ok(result.rows_affected())
    }
    async fn commit(self) -> Result<(), ReceiptCleanupError> {
        self.tx.commit().await.map_err(ReceiptCleanupError::storage)
    }
    async fn rollback(self) -> Result<(), ReceiptCleanupError> {
        self.tx
            .rollback()
            .await
            .map_err(ReceiptCleanupError::storage)
    }
}
