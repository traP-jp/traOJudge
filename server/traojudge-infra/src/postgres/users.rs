use async_trait::async_trait;
use sqlx::PgPool;
use traojudge_core::{
    domain::model::user::{PublicUserProfile, UserId},
    ports::repository::user::UserProfileRepository,
};
use uuid::Uuid;

pub struct PgUserProfileRepository {
    pool: PgPool,
}
impl PgUserProfileRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
pub(super) struct UserProfileRow {
    id: Uuid,
    display_name: String,
    bio: String,
    github_id: String,
    atcoder_id: String,
    x_id: String,
    revision: String,
}

impl From<UserProfileRow> for PublicUserProfile {
    fn from(row: UserProfileRow) -> Self {
        Self {
            id: UserId::new(row.id),
            display_name: row.display_name,
            bio: row.bio,
            github_id: row.github_id,
            atcoder_id: row.atcoder_id,
            x_id: row.x_id,
            revision: row.revision,
        }
    }
}

#[async_trait]
impl UserProfileRepository for PgUserProfileRepository {
    type Error = sqlx::Error;
    async fn find_public_profile(
        &self,
        user_id: &UserId,
    ) -> Result<Option<PublicUserProfile>, Self::Error> {
        sqlx::query_as::<_, UserProfileRow>("SELECT id, display_name, bio, github_id, atcoder_id, x_id, revision FROM users WHERE id = $1")
            .bind(user_id.as_uuid()).fetch_optional(&self.pool).await.map(|row| row.map(Into::into))
    }
}
