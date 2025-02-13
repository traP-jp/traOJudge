use axum::async_trait;

use crate::domain::entities::user::UserId;

#[async_trait]
pub trait AuthRepository {
    async fn save_user_password(&self, id: UserId, password: &str) -> anyhow::Result<()>;
    async fn update_user_password(&self, id: UserId, password: &str) -> anyhow::Result<()>;
    async fn verify_user_password(&self, id: UserId, password: &str) -> anyhow::Result<bool>;
}
