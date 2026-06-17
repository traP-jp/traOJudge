use anyhow::Context;
use sqlx::{MySqlPool, mysql::MySqlPoolOptions};

pub type MariaDbPool = MySqlPool;

#[derive(Clone, Debug)]
pub struct DatabaseConnection {
    pool: MariaDbPool,
}

impl DatabaseConnection {
    pub async fn connect(database_url: &str) -> anyhow::Result<Self> {
        let pool = MySqlPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await
            .context("failed to connect to database")?;

        Ok(Self { pool })
    }

    pub fn pool(&self) -> &MariaDbPool {
        &self.pool
    }

    pub fn clone_pool(&self) -> MariaDbPool {
        self.pool.clone()
    }
}

#[derive(Clone, Debug)]
pub struct MariaDbRepositoryProvider {
    database: DatabaseConnection,
}

impl MariaDbRepositoryProvider {
    pub fn new(database: DatabaseConnection) -> Self {
        Self { database }
    }

    pub fn database(&self) -> &DatabaseConnection {
        &self.database
    }

    pub fn pool(&self) -> &MariaDbPool {
        self.database.pool()
    }

    pub fn clone_pool(&self) -> MariaDbPool {
        self.database.clone_pool()
    }
}
