use axum::{extract::State, Json};
use lettre::Address;
use reqwest::StatusCode;
use serde::Deserialize;

use crate::{
    utils::validator::{RuleType, Validator},
    Repository,
};

#[derive(Deserialize)]
pub struct SignUpRequest {
    email: String,
}

pub async fn sign_up_request(
    State(state): State<Repository>,
    Json(body): Json<SignUpRequest>,
) -> Result<StatusCode, StatusCode> {
    let user_address = body
        .email
        .parse::<Address>()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let jwt = state
        .encode_email_signup_jwt(&body.email)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let message = format!(
        "これはテストメールです。
以下のリンクをクリックしてください。
https://link/{jwt}"
    );

    crate::utils::mail::send_email(user_address, "traOJudgeメール認証", &message)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::CREATED)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignUp {
    pub user_name: String,
    pub password: String,
    pub token: String,
}

impl Validator for SignUp {
    fn validate(&self) -> anyhow::Result<()> {
        let rules = vec![
            (&self.user_name, RuleType::UserName),
            (&self.password, RuleType::Password),
        ];
        for (field, rule) in rules {
            rule.validate(field)?;
        }
        Ok(())
    }
}

pub async fn sign_up(
    State(state): State<Repository>,
    Json(body): Json<SignUp>,
) -> Result<StatusCode, StatusCode> {
    body.validate().map_err(|_| StatusCode::BAD_REQUEST)?;
    let email = state
        .get_email_by_email_jwt(&body.token)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let id = state
        .create_user_by_email(&body.user_name, &email)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    state
        .save_raw_password(id, &body.password)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::CREATED)
}
