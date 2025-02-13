use axum::async_trait;
use sqlx::MySqlPool;
use uuid::Uuid;

use crate::domain::{
    entities::user::{UpdateUser, User, UserId, UserRole},
    repository::user::UserRepository,
};

use super::model::{UserIdRow, UserRow};

#[derive(Clone)]
pub struct UserRepositoryImpl {
    pool: MySqlPool,
}

impl UserRepositoryImpl {
    pub fn new(pool: MySqlPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepository for UserRepositoryImpl {
    async fn get_user_by_display_id(&self, display_id: i64) -> anyhow::Result<Option<User>> {
        let user = sqlx::query_as::<_, UserRow>("SELECT * FROM users WHERE display_id = ?")
            .bind(display_id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(user.map(|user| User {
            id: UserId(user.id.0),
            display_id: user.display_id,
            name: user.name,
            traq_id: user.traq_id,
            github_id: user.github_id,
            icon_url: user.icon_url,
            x_link: user.x_link,
            github_link: user.github_link,
            self_introduction: user.self_introduction,
            role: UserRole::new(user.role).unwrap(),
            created_at: user.created_at,
            updated_at: user.updated_at,
        }))
    }

    async fn get_user_by_email(&self, email: &str) -> anyhow::Result<Option<User>> {
        let user = sqlx::query_as::<_, UserRow>("SELECT * FROM users WHERE email = ?")
            .bind(email)
            .fetch_optional(&self.pool)
            .await?;

        Ok(user.map(|user| User {
            id: UserId(user.id.0),
            display_id: user.display_id,
            name: user.name,
            traq_id: user.traq_id,
            github_id: user.github_id,
            icon_url: user.icon_url,
            x_link: user.x_link,
            github_link: user.github_link,
            self_introduction: user.self_introduction,
            role: UserRole::new(user.role).unwrap(),
            created_at: user.created_at,
            updated_at: user.updated_at,
        }))
    }

    async fn create_user_by_email(&self, name: &str, email: &str) -> anyhow::Result<UserId> {
        let id = UserIdRow::new(Uuid::now_v7());

        sqlx::query("INSERT INTO users (id, name, email) VALUES (?, ?, ?)")
            .bind(id)
            .bind(name)
            .bind(email)
            .execute(&self.pool)
            .await?;

        Ok(UserId(id.0))
    }

    async fn update_user(&self, display_id: i64, body: UpdateUser) -> anyhow::Result<()> {
        sqlx::query("UPDATE users SET name = ?, icon_url = ?, x_link = ?, github_link = ?, self_introduction = ? WHERE display_id = ?")
            .bind(body.user_name)
            .bind(body.icon_url)
            .bind(body.x_link)
            .bind(body.github_link)
            .bind(body.self_introduction)
            .bind(display_id)
            .execute(&self.pool).await?;

        Ok(())
    }

    async fn is_exist_email(&self, email: &str) -> anyhow::Result<bool> {
        let user = sqlx::query_as::<_, UserRow>("SELECT * FROM users WHERE email = ?")
            .bind(email)
            .fetch_optional(&self.pool)
            .await?;

        Ok(user.is_some())
    }
}
