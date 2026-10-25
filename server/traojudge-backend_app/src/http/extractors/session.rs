use crate::http::error::ApiError;
use axum::{extract::FromRequestParts, http::request::Parts};
use traojudge_core::domain::model::account::{AccountError, AuthenticatedSession};

/// 認証済みの本人を必要とするhandlerの引数に置く。公開APIには置かない。
pub struct CurrentSession(pub AuthenticatedSession);
impl<S: Send + Sync> FromRequestParts<S> for CurrentSession {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthenticatedSession>()
            .copied()
            .map(Self)
            .ok_or_else(|| AccountError::Unauthenticated.into())
    }
}
