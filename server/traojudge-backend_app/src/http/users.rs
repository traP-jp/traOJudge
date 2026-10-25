use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use traojudge_core::{
    application::users::GetUserProfile, domain::model::user::UserId,
    ports::repository::user::UserProfileRepository,
};
use uuid::Uuid;

use super::schema::{Error, Profile};

pub(super) async fn get_user_profile<R: UserProfileRepository + 'static>(
    State(use_case): State<Arc<GetUserProfile<R>>>,
    Path(user_id): Path<String>,
) -> Response {
    let Ok(user_id) = UserId::new(user_id) else {
        return not_found();
    };

    match use_case.execute(&user_id).await {
        Ok(Some(profile)) => Json(Profile::new(
            profile.id.as_str().to_owned(),
            profile.display_name,
            profile.bio,
            profile.github_id,
            profile.atcoder_id,
            profile.x_id,
            profile.revision,
        ))
        .into_response(),
        Ok(None) => not_found(),
        Err(error) => {
            let request_id = Uuid::new_v4().to_string();
            tracing::error!(%request_id, %error, "failed to get public user profile");
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "unexpected_error",
                "処理を完了できませんでした。",
                request_id,
            )
        }
    }
}

fn not_found() -> Response {
    error_response(
        StatusCode::NOT_FOUND,
        "not_found",
        "利用者が見つかりません。",
        Uuid::new_v4().to_string(),
    )
}

fn error_response(status: StatusCode, code: &str, message: &str, request_id: String) -> Response {
    (
        status,
        Json(Error::new(code.to_owned(), message.to_owned(), request_id)),
    )
        .into_response()
}
