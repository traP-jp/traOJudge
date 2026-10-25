use async_trait::async_trait;
use sqlx::{PgPool, Row};
use traojudge_core::{
    domain::model::user::{PublicUserProfile, UserId},
    ports::repository::user::UserProfileRepository,
};

pub struct PgUserProfileRepository {
    pool: PgPool,
}

impl PgUserProfileRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserProfileRepository for PgUserProfileRepository {
    type Error = sqlx::Error;

    async fn find_public_profile(
        &self,
        user_id: &UserId,
    ) -> Result<Option<PublicUserProfile>, Self::Error> {
        let row = sqlx::query(
            "SELECT display_name, bio, github_id, atcoder_id, x_id, revision
             FROM users WHERE id = $1",
        )
        .bind(user_id.as_str())
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| {
            Ok(PublicUserProfile {
                id: user_id.clone(),
                display_name: row.try_get("display_name")?,
                bio: row.try_get("bio")?,
                github_id: row.try_get("github_id")?,
                atcoder_id: row.try_get("atcoder_id")?,
                x_id: row.try_get("x_id")?,
                revision: row.try_get("revision")?,
            })
        })
        .transpose()
    }
}
