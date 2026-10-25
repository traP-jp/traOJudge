use async_trait::async_trait;
use sqlx::PgPool;
use traojudge_core::{
    domain::model::{
        account::{AccountError, AuthenticatedSession},
        user::UserId,
    },
    ports::repository::account::SessionRepository,
};
use uuid::Uuid;

pub struct PgSessionRepository {
    pool: PgPool,
}
impl PgSessionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SessionRepository for PgSessionRepository {
    async fn refresh_session(
        &self,
        digest: &[u8],
    ) -> Result<Option<AuthenticatedSession>, AccountError> {
        let row: Option<(Uuid, Uuid)> = sqlx::query_as("UPDATE web_sessions SET expires_at = GREATEST(expires_at, clock_timestamp() + INTERVAL '90 days') WHERE token_digest = $1 AND revoked_at IS NULL AND expires_at > clock_timestamp() RETURNING id, user_id")
            .bind(digest).fetch_optional(&self.pool).await.map_err(AccountError::storage)?;
        Ok(row.map(|(id, user_id)| AuthenticatedSession {
            id,
            user_id: UserId::new(user_id),
        }))
    }
    async fn developer_token_user(&self, digest: &[u8]) -> Result<Option<UserId>, AccountError> {
        let id: Option<Uuid> = sqlx::query_scalar("SELECT user_id FROM developer_tokens WHERE token_digest = $1 AND revoked_at IS NULL AND (expires_at IS NULL OR expires_at > clock_timestamp())")
            .bind(digest).fetch_optional(&self.pool).await.map_err(AccountError::storage)?;
        Ok(id.map(UserId::new))
    }
}
