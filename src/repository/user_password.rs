use super::{users::UserId, Repository};

impl Repository {
    pub async fn save_raw_password(&self, id: UserId, password: &str) -> anyhow::Result<()> {
        let hash = bcrypt::hash(password, self.bcrypt_cost)?;

        sqlx::query("INSERT INTO user_passwords (user_id, password) VALUES (?, ?)")
            .bind(id)
            .bind(&hash)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}
