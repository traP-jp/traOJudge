use async_trait::async_trait;

use crate::domain::model::user::{PublicUserProfile, UserId};

#[async_trait]
pub trait UserProfileRepository: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    /// 存在しない利用者はNone。取得失敗と区別する。
    async fn find_public_profile(
        &self,
        user_id: &UserId,
    ) -> Result<Option<PublicUserProfile>, Self::Error>;
}
