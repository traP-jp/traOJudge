use anyhow::Result;
use async_trait::async_trait;

use crate::domain::model::user::{UpdateUser, User, UserGlobalAuthorityAttribute, UserId};

#[async_trait]
pub trait UserRepository: Send {
    async fn get_user_by_id(&mut self, id: UserId) -> Result<Option<User>>;
    async fn get_user_by_name(&mut self, name: &str) -> Result<Option<User>>;
    async fn create_user(&mut self, name: &str) -> Result<UserId>;
    async fn update_user(&mut self, id: UserId, user: UpdateUser) -> Result<()>;
    async fn get_user_global_authority_attribute(
        &mut self,
        id: UserId,
    ) -> Result<Option<UserGlobalAuthorityAttribute>>;
    async fn update_user_global_authority_attribute(
        &mut self,
        id: UserId,
        attribute: UserGlobalAuthorityAttribute,
    ) -> Result<()>;
}
