use async_session::chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};

use super::Repository;


#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
enum Action {
    reset_password,
    change_email,
}

#[derive(Serialize, Deserialize)]
struct TokenWithUserid {
    exp: i64,
    iat: i64,
    nbf: i64,
    user_id: i64,
    email: String,
    action: Action,
}

impl Repository {
    pub async fn encode_email_update_jwt(
        &self,
        user_id: i64,
        email: &str,
    ) -> anyhow::Result<String> {
        let exp = (Utc::now() + Duration::minutes(60)).timestamp();
        let iat = Utc::now().timestamp();
        let nbf = Utc::now().timestamp();

        let claims = TokenWithUserid {
            exp,
            iat,
            nbf,
            user_id,
            email: email.to_owned(),
            action: Action::change_email,
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
