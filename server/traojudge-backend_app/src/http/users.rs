use super::{error::ApiError, extractors::ResourceId, schema::Profile};
use axum::{Json, Router, extract::State, routing::get};
use std::sync::Arc;
use traojudge_core::{
    application::users::GetUserProfile,
    domain::model::{account::AccountError, user::UserId},
    ports::repository::user::UserProfileRepository,
};

pub fn router<R: UserProfileRepository + 'static>(use_case: GetUserProfile<R>) -> Router {
    Router::new()
        .route("/{userId}", get(get_user_profile::<R>))
        .with_state(Arc::new(use_case))
}
async fn get_user_profile<R: UserProfileRepository>(
    State(use_case): State<Arc<GetUserProfile<R>>>,
    ResourceId(id): ResourceId,
) -> Result<Json<Profile>, ApiError> {
    let profile = use_case
        .execute(&UserId::new(id))
        .await
        .map_err(ApiError::storage)?
        .ok_or(AccountError::NotFound)?;
    Ok(Json(profile.into()))
}
