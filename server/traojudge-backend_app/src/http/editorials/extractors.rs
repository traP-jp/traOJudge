use crate::http::error::ApiError;
use axum::{
    extract::{FromRequestParts, Path, Query},
    http::request::Parts,
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::DateTime;
use serde::Deserialize;
use traojudge_core::domain::model::editorial::{
    EditorialCreationKey, EditorialCursor, EditorialError, EditorialPageRequest,
};
use uuid::Uuid;

pub(super) struct CreationKey(pub EditorialCreationKey);
impl<S: Send + Sync> FromRequestParts<S> for CreationKey {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let mut values = parts.headers.get_all("idempotency-key").iter();
        let value = values.next().ok_or(ApiError::BadRequest)?;
        if values.next().is_some() {
            return Err(ApiError::BadRequest);
        }
        EditorialCreationKey::parse(value.to_str().map_err(|_| ApiError::BadRequest)?)
            .map(Self)
            .map_err(|_| ApiError::BadRequest)
    }
}

pub(super) struct AuthorIds(pub Uuid, pub Uuid);
impl<S: Send + Sync> FromRequestParts<S> for AuthorIds {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path((editorial_id, user_id)) = Path::<(Uuid, Uuid)>::from_request_parts(parts, state)
            .await
            .map_err(|_| EditorialError::NotFound)?;
        Ok(Self(editorial_id, user_id))
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PageQuery {
    limit: Option<u32>,
    cursor: Option<String>,
}
pub(super) struct Page(pub EditorialPageRequest);
impl<S: Send + Sync> FromRequestParts<S> for Page {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Query(query) = Query::<PageQuery>::from_request_parts(parts, state)
            .await
            .map_err(|_| ApiError::BadRequest)?;
        Ok(Self(EditorialPageRequest {
            limit: query.limit.unwrap_or(30),
            cursor: query.cursor.as_deref().map(decode_cursor).transpose()?,
        }))
    }
}
pub(super) fn encode_cursor(cursor: EditorialCursor) -> String {
    URL_SAFE_NO_PAD.encode(format!(
        "{}|{}|{}",
        cursor.problem_id,
        cursor.created_at.to_rfc3339(),
        cursor.id
    ))
}
fn decode_cursor(value: &str) -> Result<EditorialCursor, EditorialError> {
    let invalid = || EditorialError::validation("cursor", "invalid cursor");
    if value.is_empty() || value.len() > 2048 {
        return Err(invalid());
    }
    let decoded = URL_SAFE_NO_PAD.decode(value).map_err(|_| invalid())?;
    let decoded = String::from_utf8(decoded).map_err(|_| invalid())?;
    let parts = decoded.split('|').collect::<Vec<_>>();
    let [problem_id, created_at, id] = parts.as_slice() else {
        return Err(invalid());
    };
    Ok(EditorialCursor {
        problem_id: problem_id.parse().map_err(|_| invalid())?,
        created_at: DateTime::parse_from_rfc3339(created_at)
            .map_err(|_| invalid())?
            .to_utc(),
        id: id.parse().map_err(|_| invalid())?,
    })
}
