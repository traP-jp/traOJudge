use async_session::chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};

use super::Repository;

#[derive(Serialize, Deserialize)]
struct SignupClaims {
    user_id: i64,
    email: String,
    exp: i64,
}

impl Repository {
    pub async fn encode_email_update_jwt(
        &self,
        user_id: i64,
        email: &str,
    ) -> anyhow::Result<String> {
        let exp = (Utc::now() + Duration::minutes(60)).timestamp();
        let claims = SignupClaims {
            user_id: user_id,
            email: email.to_owned(),
            exp,
        };

        let encode_key: String = std::env::var("JWT_SECRET")?;

        let jwt = jsonwebtoken::encode(
            &jsonwebtoken::Header::default(),
            &claims,
            &jsonwebtoken::EncodingKey::from_secret(encode_key.as_ref()),
        )?;

        Ok(jwt)
    }
}
