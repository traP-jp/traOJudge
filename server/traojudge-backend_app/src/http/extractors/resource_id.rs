use crate::http::error::ApiError;
use axum::{
    extract::{FromRequestParts, Path},
    http::request::Parts,
};
use traojudge_core::domain::model::account::AccountError;
use uuid::Uuid;

pub struct ResourceId(pub Uuid);
impl<S: Send + Sync> FromRequestParts<S> for ResourceId {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path(value) = Path::<String>::from_request_parts(parts, state)
            .await
            .map_err(|_| AccountError::NotFound)?;
        value
            .parse()
            .map(Self)
            .map_err(|_| AccountError::NotFound.into())
    }
}
