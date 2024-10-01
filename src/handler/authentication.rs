use axum::{extract::State, Json};
use lettre::Address;
use reqwest::StatusCode;
use serde::Deserialize;

use crate::repository::Repository;

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
