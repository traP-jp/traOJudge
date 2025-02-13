use async_sqlx_session::MySqlSessionStore;
use sqlx::{
    migrate,
    mysql::{MySqlConnectOptions, MySqlPoolOptions},
    MySqlPool,
};

use super::{
    external::mail::MailClientImpl,
    repository::{
        auth::AuthRepositoryImpl, session::SessionRepositoryImpl, user::UserRepositoryImpl,
    },
};

pub struct Provider {
    pool: MySqlPool,
    session_store: MySqlSessionStore,
    bcrypt_cost: u32,
}

impl Provider {
    pub async fn new() -> anyhow::Result<Self> {
        let options = get_option_from_env()?;

        let pool = MySqlPoolOptions::new()
            .max_connections(10)
            .connect_with(options)
            .await?;

        let session_store =
            MySqlSessionStore::from_client(pool.clone()).with_table_name("user_sessions");

        migrate!("./migrations").run(&pool).await?;
        session_store.migrate().await?;

        Ok(Self {
            pool,
            session_store,
            bcrypt_cost: bcrypt::DEFAULT_COST,
        })
    }

    pub fn provide_auth_repository(&self) -> AuthRepositoryImpl {
        AuthRepositoryImpl::new(self.bcrypt_cost, self.pool.clone())
    }

    pub fn provide_session_repository(&self) -> SessionRepositoryImpl {
        SessionRepositoryImpl::new(self.session_store.clone())
    }

    pub fn provide_user_repository(&self) -> UserRepositoryImpl {
        UserRepositoryImpl::new(self.pool.clone())
    }

    pub fn provide_mail_client(&self) -> MailClientImpl {
        MailClientImpl::new().unwrap()
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
