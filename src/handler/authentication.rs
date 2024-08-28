use axum::{extract::State, Json};
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

    let jwt = state
        .make_jwt_and_save(&body.email)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::BAD_REQUEST)?;

    // メール送る
    

    Ok(StatusCode::CREATED)
}