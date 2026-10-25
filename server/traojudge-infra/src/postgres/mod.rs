//! PostgreSQLによる永続化・トランザクションとmigration。

use std::time::Duration;

use sqlx::{PgPool, migrate::Migrator, postgres::PgPoolOptions};

pub mod users;

pub static MIGRATOR: Migrator = sqlx::migrate!("./src/postgres/migrations");

pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(database_url)
        .await
}
