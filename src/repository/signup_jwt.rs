use async_session::chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};

use super::Repository;

#[derive(Serialize, Deserialize)]
struct SignupClaims {
    exp: i64,
    iat: i64,
    email: String,
}

impl Repository {
    pub async fn encode_email_signup_jwt(
        &self,
        email: &str,
    ) -> anyhow::Result<String> {
        let exp = (Utc::now() + Duration::minutes(60)).timestamp();
        let iat = Utc::now().timestamp();


        let claims = SignupClaims {
            exp,
            iat,
            email: email.to_owned(),
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
