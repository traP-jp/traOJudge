use async_sqlx_session::MySqlSessionStore;
use aws_config::Region;
use aws_sdk_s3::{config::Credentials, Config};
use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions};

use super::Repository;

mod jwt;
mod object_storage;
mod user_password;
pub mod users;
mod users_session;

impl Repository {
    pub async fn connect() -> anyhow::Result<Self> {
        let options = get_option_from_env()?;

        let pool = MySqlPoolOptions::new()
            .max_connections(10)
            .connect_with(options)
            .await?;

        let session_store =
            MySqlSessionStore::from_client(pool.clone()).with_table_name("user_sessions");

        let config = get_config_from_env()?;
        let s3_client = aws_sdk_s3::Client::from_conf(config);
        let bucket_name = std::env::var("OBJECT_STORAGE_BUCKET")?;
        let _ = s3_client.create_bucket().bucket(&bucket_name).send().await;

        Ok(Self {
            pool,
            session_store,
            s3_client,
            bucket_name,
            bcrypt_cost: bcrypt::DEFAULT_COST,
        })
    }

    pub async fn migrate(&self) -> anyhow::Result<()> {
        sqlx::migrate!("./migrations").run(&self.pool).await?;

        self.session_store.migrate().await?;

        Ok(())
    }

    pub async fn create_by_pool(pool: sqlx::MySqlPool) -> anyhow::Result<Self> {
        let session_store =
            MySqlSessionStore::from_client(pool.clone()).with_table_name("user_sessions");
        session_store.migrate().await?;

        let config = get_config_from_env()?;
        let s3_client = aws_sdk_s3::Client::from_conf(config);
        let bucket_name = std::env::var("OBJECT_STORAGE_BUCKET")?;
        let _ = s3_client.create_bucket().bucket(&bucket_name).send().await;

        Ok(Self {
            pool,
            session_store,
            s3_client,
            bucket_name,
            bcrypt_cost: bcrypt::DEFAULT_COST,
        })
    }
}

fn get_option_from_env() -> anyhow::Result<MySqlConnectOptions> {
    let host = std::env::var("DB_HOSTNAME")?;
    let port = std::env::var("DB_PORT")?
        .parse()
        .map_err(|_| anyhow::anyhow!("DB_PORT must be a number"))?;
    let user = std::env::var("DB_USERNAME")?;
    let password = std::env::var("DB_PASSWORD")?;
    let db_name = std::env::var("DB_DATABASE")?;

    Ok(MySqlConnectOptions::new()
        .host(&host)
        .port(port)
        .username(&user)
        .password(&password)
        .database(&db_name))
}

fn get_config_from_env() -> anyhow::Result<Config> {
    let access_key = std::env::var("OBJECT_STORAGE_ACCESS_KEY")?;
    let secret_key = std::env::var("OBJECT_STORAGE_SECRET_KEY")?;
    let region = std::env::var("OBJECT_STORAGE_REGION")?;
    let endpoint = std::env::var("OBJECT_STORAGE_ENDPOINT")?;

    let credentials_provider = Credentials::new(access_key, secret_key, None, None, "static");
    let config = aws_sdk_s3::Config::builder()
        .behavior_version_latest()
        .credentials_provider(credentials_provider)
        .region(Region::new(region))
        .force_path_style(true)
        .endpoint_url(endpoint)
        .build();

    Ok(config)
}
