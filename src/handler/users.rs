use axum::{extract::State, response::IntoResponse, Json};
use axum_extra::{headers::Cookie, TypedHeader};
use lettre::Address;
use reqwest::StatusCode;
use serde::Deserialize;
use validator::Validate;

use crate::repository::users::PutMeRequest;
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
    TypedHeader(cookie): TypedHeader<Cookie>,
    Json(body): Json<EmailUpdate>,
) -> anyhow::Result<StatusCode, StatusCode> {
    let session_id = cookie.get("session_id").ok_or(StatusCode::UNAUTHORIZED)?;

    let user_id = state
        .get_user_id_by_session_id(session_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let email = body
        .email
        .parse::<Address>()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let jwt = state
        .encode_email_update_jwt(user_id, &body.email)
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

pub async fn put_me(
    State(state): State<Repository>,
    TypedHeader(cookie): TypedHeader<Cookie>,
    Json(body): Json<PutMeRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    body.validate().map_err(|_| StatusCode::BAD_REQUEST)?;

    let session_id = cookie.get("session_id").ok_or(StatusCode::UNAUTHORIZED)?;

    let user_id = state
        .get_user_id_by_session_id(session_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // todo: icon -> icon_url
    let new_body = body.clone();

    // iconの値がない場合空文字列を返すことになるが、この挙動はよくない気がする
    let icon_url = new_body.icon.clone().unwrap_or_default();

    state
        .update_user(user_id, new_body)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({"iconUrl": icon_url})))
}
