use anyhow::Result;
use async_trait::async_trait;

use crate::domain::model::{session::SessionUser, user::UserId};

#[async_trait]
pub trait SessionRepository: Send {
    async fn create_session(&mut self, user_id: UserId) -> Result<String>;
    async fn delete_session(&mut self, session_id: &str) -> Result<bool>;
    async fn get_session_user(&mut self, session_id: &str) -> Result<Option<SessionUser>>;
}
