use axum::async_trait;

use crate::domain::entities::user::User;

#[async_trait]
pub trait SessionRepository {
    async fn create_session(&self, user: User) -> anyhow::Result<String>;
    async fn delete_session(&self, session_id: &str) -> anyhow::Result<Option<()>>;
    async fn get_user_id_by_session_id(&self, session_id: &str) -> anyhow::Result<Option<String>>;
    async fn get_display_id_by_session_id(&self, session_id: &str) -> anyhow::Result<Option<i64>>;
}
