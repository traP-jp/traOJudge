use axum::{extract::Path, extract::State, response::IntoResponse, Json};
use axum_extra::{headers::Cookie, TypedHeader};
use lettre::Address;
use reqwest::StatusCode;
use serde::Deserialize;

use crate::repository::users::UpdateUser;
use crate::repository::Repository;
use crate::utils::validator::{RuleType, Validator};

#[derive(Deserialize)]
pub struct EmailUpdate {
    email: String,
}

pub async fn get_me(
    State(state): State<Repository>,
    TypedHeader(cookie): TypedHeader<Cookie>,
) -> anyhow::Result<impl IntoResponse, StatusCode> {
    let session_id = cookie.get("session_id").ok_or(StatusCode::UNAUTHORIZED)?;

    let display_id = state
        .get_display_id_by_session_id(session_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let user = state
        .get_user_by_display_id(display_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(user))
}

pub async fn put_me_email(
    State(state): State<Repository>,
    TypedHeader(cookie): TypedHeader<Cookie>,
    Json(body): Json<EmailUpdate>,
) -> anyhow::Result<StatusCode, StatusCode> {
    let session_id = cookie.get("session_id").ok_or(StatusCode::UNAUTHORIZED)?;

    let display_id = state
        .get_display_id_by_session_id(session_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let email = body
        .email
        .parse::<Address>()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let jwt = state
        .encode_email_update_jwt(display_id, &body.email)
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

#[derive(serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PutMeRequest {
    pub user_name: Option<String>,
    pub icon: Option<String>,
    pub x_link: Option<String>,
    pub github_link: Option<String>,
    pub self_introduction: Option<String>,
}

impl Validator for PutMeRequest {
    fn validate(&self) -> anyhow::Result<()> {
        let rules = vec![
            (&self.user_name, RuleType::UserName),
            (&self.icon, RuleType::Icon),
            (&self.x_link, RuleType::XLink),
            (&self.github_link, RuleType::GitHubLink),
            (&self.self_introduction, RuleType::SelfIntroduction),
        ];
        for (value, rule) in rules {
            if let Some(value) = value {
                rule.validate(value)?;
            }
        }
        Ok(())
    }
}

// todo とりえずの仮置き
fn encode_icon_to_icon_url(icon: Option<String>) -> Option<String> {
    icon
}

pub async fn put_me(
    State(state): State<Repository>,
    TypedHeader(cookie): TypedHeader<Cookie>,
    Json(body): Json<PutMeRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    body.validate().map_err(|_| StatusCode::BAD_REQUEST)?;

    let session_id = cookie.get("session_id").ok_or(StatusCode::UNAUTHORIZED)?;

    let display_id = state
        .get_display_id_by_session_id(session_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let user = state
        .get_user_by_display_id(display_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let new_body = UpdateUser {
        user_name: body.user_name.unwrap_or(user.name),
        icon_url: body
            .icon
            .map_or(user.icon_url, |icon| encode_icon_to_icon_url(Some(icon))),
        x_link: body.x_link.or(user.x_link),
        github_link: body.github_link.or(user.github_link),
        self_introduction: body.self_introduction.unwrap_or(user.self_introduction),
    };

    let icon_url = new_body.icon_url.clone();

    state
        .update_user(user.display_id, new_body)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(serde_json::json!({"iconUrl": icon_url})))
}

pub async fn get_user(
    State(state): State<Repository>,
    Path(display_id): Path<String>,
) -> anyhow::Result<impl IntoResponse, StatusCode> {
    let display_id = display_id
        .parse::<i64>()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let user = state
        .get_user_by_display_id(display_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(user))
}
