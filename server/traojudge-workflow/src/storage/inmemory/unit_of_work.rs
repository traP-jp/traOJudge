use async_trait::async_trait;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::storage::{UnitOfWork, UnitOfWorkProvider};

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

    async fn commit(self) -> Result<(), Self::Error> {
        self.transaction.commit().await
    }

    async fn rollback(self) -> Result<(), Self::Error> {
        self.transaction.rollback().await
    }
}

#[cfg(test)]
mod tests;
