use async_session::chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};

use super::Repository;

#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
enum Action {
    register_email
}

#[derive(Serialize, Deserialize)]
struct TokenWithoutUserid {
    exp: i64,
    iat: i64,
    nbf: i64,
    email: String,
    action: Action,
}


impl Repository {
    pub async fn encode_email_signup_jwt(
        &self,
        email: &str,
    ) -> anyhow::Result<String> {
        let exp = (Utc::now() + Duration::minutes(60)).timestamp();
        let iat = Utc::now().timestamp();
        let nbf = Utc::now().timestamp();

        let claims = TokenWithoutUserid {
            exp,
            iat,
            nbf,
            email: email.to_owned(),
            action: Action::register_email,
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
