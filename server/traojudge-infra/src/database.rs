use anyhow::Context;
use sqlx::{
    MySql, MySqlPool,
    mysql::{MySqlConnection, MySqlPoolOptions},
    pool::PoolConnection,
};

use crate::repository::user::MariaDbUserRepository;

pub type MariaDbPool = MySqlPool;

#[derive(Debug)]
pub struct MariaDbConnection {
    connection: PoolConnection<MySql>,
}

impl MariaDbConnection {
    pub fn connection(&mut self) -> &mut MySqlConnection {
        self.connection.as_mut()
    }

    pub fn provide_user_repository(&mut self) -> MariaDbUserRepository<'_> {
        MariaDbUserRepository::new(self.connection())
    }
}

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

    pub async fn migrate(&self) -> anyhow::Result<()> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .context("failed to run database migrations")?;

        Ok(())
    }

    pub fn pool(&self) -> &MariaDbPool {
        &self.pool
    }

    pub fn clone_pool(&self) -> MariaDbPool {
        self.pool.clone()
    }

    pub async fn acquire(&self) -> anyhow::Result<MariaDbConnection> {
        let connection = self
            .pool
            .acquire()
            .await
            .context("failed to acquire database connection")?;

        Ok(MariaDbConnection { connection })
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

    pub async fn acquire(&self) -> anyhow::Result<MariaDbConnection> {
        self.database.acquire().await
    }
}
