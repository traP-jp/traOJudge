use crate::{
    domain::{
        entities::user::{User, UserId},
        repository::user::UserRepository,
    },
    usecase::model::user::UpdateUserRequest,
};

#[derive(Clone)]
pub struct UserService<R: UserRepository> {
    repository: R,
}

impl<R: UserRepository> UserService<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }
}

impl<R: UserRepository> UserService<R> {
    async fn get_user_by_display_id(&self, display_id: i64) -> anyhow::Result<Option<User>> {
        self.repository.get_user_by_display_id(display_id).await
    }

    async fn get_user_by_email(&self, email: &str) -> anyhow::Result<Option<User>> {
        self.repository.get_user_by_email(email).await
    }

    async fn create_user_by_email(&self, name: &str, email: &str) -> anyhow::Result<UserId> {
        self.repository.create_user_by_email(name, email).await
    }

    async fn update_user(&self, display_id: i64, body: UpdateUserRequest) -> anyhow::Result<()> {
        self.repository.update_user(display_id, body.into()).await
    }

    async fn is_exist_email(&self, email: &str) -> anyhow::Result<bool> {
        self.repository.is_exist_email(email).await
    }
}
