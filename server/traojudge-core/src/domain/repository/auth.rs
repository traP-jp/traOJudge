use anyhow::Result;
use async_trait::async_trait;

use crate::domain::model::{auth::UserAuthentication, user::UserId};

#[async_trait]
pub trait AuthenticationMethodRepository: Send {
    async fn count_authentication_methods(&mut self, user_id: UserId) -> Result<i64>;
    async fn get_authentication_by_user_id(
        &mut self,
        user_id: UserId,
    ) -> Result<UserAuthentication>;
}

#[async_trait]
pub trait EmailPasswordRepository: Send {
    async fn save_user_email_and_password(
        &mut self,
        user_id: UserId,
        email: &str,
        password: &str,
    ) -> Result<()>;
    async fn update_user_password(&mut self, user_id: UserId, password: &str) -> Result<()>;
    async fn verify_user_password(&mut self, user_id: UserId, password: &str) -> Result<bool>;
    async fn update_user_email(&mut self, user_id: UserId, email: &str) -> Result<()>;
    async fn get_user_id_by_email(&mut self, email: &str) -> Result<Option<UserId>>;
    async fn is_exist_email(&mut self, email: &str) -> Result<bool>;
}

#[async_trait]
pub trait GoogleOAuthRepository: Send {
    async fn get_google_oauth2_url(&mut self, oauth_action: &str) -> Result<String>;
    async fn get_google_oauth_by_authorize_code(
        &mut self,
        code: &str,
        oauth_action: &str,
    ) -> Result<String>;
    async fn save_user_google_oauth(&mut self, user_id: UserId, google_oauth: &str) -> Result<()>;
    async fn update_user_google_oauth(&mut self, user_id: UserId, google_oauth: &str)
    -> Result<()>;
    async fn verify_user_google_oauth(&mut self, user_id: UserId) -> Result<bool>;
    async fn delete_user_google_oauth(&mut self, user_id: UserId) -> Result<bool>;
    async fn get_user_id_by_google_oauth(&mut self, google_oauth: &str) -> Result<Option<UserId>>;
}

#[async_trait]
pub trait GitHubOAuthRepository: Send {
    async fn get_github_oauth2_url(&mut self, oauth_action: &str) -> Result<String>;
    async fn get_github_oauth_by_authorize_code(
        &mut self,
        code: &str,
        oauth_action: &str,
    ) -> Result<String>;
    async fn save_user_github_oauth(&mut self, user_id: UserId, github_oauth: &str) -> Result<()>;
    async fn update_user_github_oauth(&mut self, user_id: UserId, github_oauth: &str)
    -> Result<()>;
    async fn verify_user_github_oauth(&mut self, user_id: UserId) -> Result<bool>;
    async fn delete_user_github_oauth(&mut self, user_id: UserId) -> Result<bool>;
    async fn get_user_id_by_github_oauth(&mut self, github_oauth: &str) -> Result<Option<UserId>>;
}

#[async_trait]
pub trait TraqOAuthRepository: Send {
    async fn save_user_traq_oauth(&mut self, user_id: UserId, traq_oauth: &str) -> Result<()>;
    async fn update_user_traq_oauth(&mut self, user_id: UserId, traq_oauth: &str) -> Result<()>;
    async fn verify_user_traq_oauth(&mut self, user_id: UserId) -> Result<bool>;
    async fn delete_user_traq_oauth(&mut self, user_id: UserId) -> Result<bool>;
    async fn get_user_id_by_traq_oauth(&mut self, traq_oauth: &str) -> Result<Option<UserId>>;
}

pub trait AuthRepository:
    AuthenticationMethodRepository
    + EmailPasswordRepository
    + GoogleOAuthRepository
    + GitHubOAuthRepository
    + TraqOAuthRepository
{
}

impl<T> AuthRepository for T where
    T: AuthenticationMethodRepository
        + EmailPasswordRepository
        + GoogleOAuthRepository
        + GitHubOAuthRepository
        + TraqOAuthRepository
{
}
