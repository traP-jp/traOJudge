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

pub struct UpdateUser {
    pub user_name: String,
    pub icon_url: String,
    pub x_link: Option<String>,
    pub github_link: Option<String>,
    pub self_introduction: String,
}

impl Repository {
    pub async fn get_user_by_id(&self, user_id: i64) -> anyhow::Result<Option<User>> {
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(user)
    }

    pub async fn update_user(&self, user_id: i64, body: UpdateUser) -> anyhow::Result<()> {
        sqlx::query("UPDATE users SET name = ?, icon_url = ?, x_link = ?, github_link = ?, self_introduction = ? WHERE id = ?")
            .bind(body.user_name)
            .bind(body.icon_url)
            .bind(body.x_link)
            .bind(body.github_link)
            .bind(body.self_introduction)
            .bind(user_id)
            .execute(&self.pool).await?;
        Ok(())
    }
}
