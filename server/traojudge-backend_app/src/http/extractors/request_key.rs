use crate::http::error::ApiError;
use axum::{extract::FromRequestParts, http::request::Parts};
use traojudge_core::domain::model::request::IdempotencyKey;
pub struct RequestKey(pub IdempotencyKey);
impl<S: Send + Sync> FromRequestParts<S> for RequestKey {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let mut values = parts.headers.get_all("idempotency-key").iter();
        let value = values.next().ok_or(ApiError::BadRequest)?;
        if values.next().is_some() {
            return Err(ApiError::BadRequest);
        }
        IdempotencyKey::parse(value.to_str().map_err(|_| ApiError::BadRequest)?)
            .map(Self)
            .map_err(|_| ApiError::BadRequest)
    }
}
