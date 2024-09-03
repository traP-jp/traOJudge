use async_session::chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};

use super::Repository;

impl Repository {
    pub async fn save_email_varifications(&self, email: &str) -> anyhow::Result<String> {
        let jwt = encode_jwt_from_email(email)?;

        sqlx::query("INSERT INTO mail_verifications (email, token) VALUES (?, ?)")
            .bind(email)
            .bind(&jwt)
            .execute(&self.pool)
            .await?;

        Ok(jwt)
    }
}

#[derive(Serialize, Deserialize)]
struct SignupClaims {
    email: String,
    exp: i64,
}

fn encode_jwt_from_email(email: &str) -> anyhow::Result<String> {
    let exp = (Utc::now() + Duration::minutes(60)).timestamp();
    let claims = SignupClaims {
        email: email.to_owned(),
        exp,
    };

    let encode_key = std::env::var("JWT_SECRET")?;

    let jwt = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(encode_key.as_ref()),
    )?;

    Ok(jwt)
}
