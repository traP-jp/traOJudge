use serde::Serialize;
use sqlx::{types::chrono, FromRow};

use super::Repository;

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, sqlx::Type, Serialize)]
#[repr(i32)]
pub enum UserRole {
    common_user = 0,
    traP_user = 1,
    admin = 2,
}

#[derive(Debug, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: i64,
    pub name: String,
    pub traq_id: Option<String>,
    pub github_id: Option<String>,
    pub icon_url: String,
    pub x_link: Option<String>,
    pub github_link: Option<String>,
    pub self_introduction: String,
    pub role: UserRole,
    // todo: add more fields
    //pub post_problems:
    //pub submut_problems:
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl Repository {
    pub async fn get_user_by_id(&self, user_id: i64) -> anyhow::Result<User> {
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_one(&self.pool)
            .await?;

        Ok(user)
    }
}
