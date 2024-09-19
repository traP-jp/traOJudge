use axum::{extract::State, response::IntoResponse, Json};
use axum_extra::{headers::Cookie, TypedHeader};
use lettre::Address;
use reqwest::StatusCode;
use serde::Deserialize;

use crate::repository::Repository;

#[derive(Deserialize)]
pub struct EmailUpdate {
    email: String,
}

pub async fn get_me(
    State(state): State<Repository>,
    TypedHeader(cookie): TypedHeader<Cookie>,
) -> anyhow::Result<impl IntoResponse, StatusCode> {
    let session_id = cookie.get("session_id").ok_or(StatusCode::UNAUTHORIZED)?;

    let user_id = state
        .get_user_id_by_session_id(session_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let user = state
        .get_user_by_id(user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(user))
}

pub async fn put_me_email(
    State(state): State<Repository>,
    Json(body): Json<EmailUpdate>,
) -> anyhow::Result<StatusCode, StatusCode> {
    let email = body
        .email
        .parse::<Address>()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let jwt = state
        .save_email_varifications(&body.email)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let message = format!(
        "以下のリンクをクリックして、メールアドレスの変更を確認してください。
https://link/{jwt}"
    );

    crate::utils::mail::send_email(email, "「traOJudge」メール変更の確認", &message)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
}
