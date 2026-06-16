use anyhow::Result;
use async_trait::async_trait;

use crate::domain::model::{session::SessionUser, user::User};

#[async_trait]
pub trait SessionRepository: Send {
    async fn create_session(&mut self, user: User) -> Result<String>;
    async fn delete_session(&mut self, session_id: &str) -> Result<Option<()>>;
    async fn get_session_user(&mut self, session_id: &str) -> Result<Option<SessionUser>>;
}
