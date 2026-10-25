use crate::{
    domain::model::user::{PublicUserProfile, UserId},
    ports::repository::user::UserProfileRepository,
};

pub struct GetUserProfile<R> {
    repository: R,
}

impl<R: UserProfileRepository> GetUserProfile<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    /// 公開プロフィールの参照には認証・役割を要求しない。
    pub async fn execute(&self, user_id: &UserId) -> Result<Option<PublicUserProfile>, R::Error> {
        self.repository.find_public_profile(user_id).await
    }
}
