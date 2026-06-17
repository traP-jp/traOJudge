use traojudge_infra::{
    database::{DatabaseConnection, MariaDbRepositoryProvider},
    unit_of_work::MariaDbUnitOfWorkProvider,
};

use crate::config::Config;

#[derive(Clone, Debug)]
pub struct AppState<P = MariaDbUnitOfWorkProvider> {
    provider: P,
}

impl AppState<MariaDbUnitOfWorkProvider> {
    pub async fn from_config(config: &Config) -> anyhow::Result<Self> {
        let database = DatabaseConnection::connect(&config.database_url).await?;
        let provider = MariaDbUnitOfWorkProvider::new(database);

        Ok(Self::new(provider))
    }

    pub fn repository_provider(&self) -> MariaDbRepositoryProvider {
        self.provider.provide_repository_provider()
    }

    pub fn unit_of_work_provider(&self) -> &MariaDbUnitOfWorkProvider {
        &self.provider
    }
}

impl<P> AppState<P> {
    pub fn new(provider: P) -> Self {
        Self { provider }
    }

    pub fn provider(&self) -> &P {
        &self.provider
    }
}
