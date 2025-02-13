use uuid::Uuid;

use crate::domain::{entities::user::UserId, repository::auth::AuthRepository};

#[derive(Clone)]
pub struct AuthenticationService<R>
where
    R: AuthRepository,
{
    repository: R,
}

impl<R: AuthRepository> AuthenticationService<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }
}

impl<R: AuthRepository> AuthenticationService<R> {
    pub async fn save_user_password(&self, id: Uuid, password: &str) -> anyhow::Result<()> {
        self.repository
            .save_user_password(UserId(id), password)
            .await
    }

    pub async fn update_user_password(&self, id: Uuid, password: &str) -> anyhow::Result<()> {
        self.repository
            .update_user_password(UserId(id), password)
            .await
    }

    pub async fn verify_user_password(&self, id: Uuid, password: &str) -> anyhow::Result<bool> {
        self.repository
            .verify_user_password(UserId(id), password)
            .await
    }
}
