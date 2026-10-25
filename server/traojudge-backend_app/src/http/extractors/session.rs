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

/// 公開取得にも有効なセッションがあれば本人の閲覧権限を渡す。
pub struct OptionalSession(pub Option<AuthenticatedSession>);
impl<S: Send + Sync> FromRequestParts<S> for OptionalSession {
    type Rejection = std::convert::Infallible;
    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        Ok(Self(
            parts.extensions.get::<AuthenticatedSession>().copied(),
        ))
    }
}
