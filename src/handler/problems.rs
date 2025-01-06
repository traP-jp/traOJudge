use axum::{extract::Path, extract::State, response::IntoResponse, Json};
use axum_extra::{headers::Cookie, TypedHeader};
use lettre::Address;
use reqwest::StatusCode;
use serde::Deserialize;

use super::Repository;
use crate::repository::problems::Problem;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostProblemRequest {
    pub title: String,
    pub difficulty: i64,
    pub statement: String,
    pub time_limit: i64,
    pub memory_limit: i64,
}

pub async fn post_problems(
    State(state): State<Repository>,
    TypedHeader(cookie): TypedHeader<Cookie>,
    Json(body): Json<PostProblemRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let session_id = cookie.get("session_id").ok_or(StatusCode::UNAUTHORIZED)?;

    Ok(())
}
