use crate::http::error::ApiError;
use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use traojudge_core::domain::model::account::AccountError;

pub fn etag(revision: &str) -> String {
    format!("\"{}\"", URL_SAFE_NO_PAD.encode(revision))
}
pub struct IfMatch(pub String);
impl<S: Send + Sync> FromRequestParts<S> for IfMatch {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let mut values = parts.headers.get_all(header::IF_MATCH).iter();
        let value = values.next().ok_or(ApiError::PreconditionRequired)?;
        if values.next().is_some() {
            return Err(ApiError::BadRequest);
        }
        let value = value.to_str().map_err(|_| ApiError::BadRequest)?;
        let encoded = value
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
            .ok_or(ApiError::BadRequest)?;
        if encoded.is_empty() || encoded.contains('"') {
            return Err(ApiError::BadRequest);
        }
        let revision = URL_SAFE_NO_PAD
            .decode(encoded)
            .map_err(|_| AccountError::RevisionMismatch)?;
        let revision = String::from_utf8(revision).map_err(|_| AccountError::RevisionMismatch)?;
        if !(1..=256).contains(&revision.chars().count()) {
            return Err(AccountError::RevisionMismatch.into());
        }
        Ok(Self(revision))
    }
}
