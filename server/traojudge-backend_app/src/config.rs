use std::net::SocketAddr;

use anyhow::Context;

pub const ADDR_ENV: &str = "TRAOJUDGE_ADDR";
pub const DATABASE_URL_ENV: &str = "TRAOJUDGE_DATABASE_URL";
pub const FRONTEND_URL_ENV: &str = "TRAOJUDGE_FRONTEND_URL";
pub const LEGACY_FRONTEND_URL_ENV: &str = "FRONTEND_URL";

const DEFAULT_ADDR: &str = "127.0.0.1:3000";
const DEFAULT_DATABASE_URL: &str = "mysql://traojudge:traojudge@127.0.0.1:3307/traojudge";
const DEFAULT_FRONTEND_ORIGIN: &str = "http://localhost:5173";

#[derive(Debug, Clone)]
pub struct Config {
    pub addr: SocketAddr,
    pub database_url: String,
    pub frontend_origins: Vec<String>,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let addr = std::env::var(ADDR_ENV)
            .unwrap_or_else(|_| DEFAULT_ADDR.to_owned())
            .parse::<SocketAddr>()
            .with_context(|| format!("{ADDR_ENV} must be a socket address"))?;

        let database_url =
            std::env::var(DATABASE_URL_ENV).unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_owned());

        let mut frontend_origins = vec![DEFAULT_FRONTEND_ORIGIN.to_owned()];

        for env_name in [FRONTEND_URL_ENV, LEGACY_FRONTEND_URL_ENV] {
            if let Ok(origin) = std::env::var(env_name)
                && !frontend_origins.contains(&origin)
            {
                frontend_origins.push(origin);
            }
        }

        Ok(Self {
            addr,
            database_url,
            frontend_origins,
        })
    }
}
