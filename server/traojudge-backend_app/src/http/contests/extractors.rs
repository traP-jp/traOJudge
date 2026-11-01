use crate::http::error::ApiError;
use axum::{
    extract::{FromRequestParts, Path, Query},
    http::request::Parts,
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use traojudge_core::domain::model::contest::{ContestError, ContestKind, ContestPageRequest};
use uuid::Uuid;

pub(super) struct ResourcePair(pub Uuid, pub Uuid);
impl<S: Send + Sync> FromRequestParts<S> for ResourcePair {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path((id, child)) = Path::<(Uuid, Uuid)>::from_request_parts(parts, state)
            .await
            .map_err(|_| ContestError::NotFound)?;
        Ok(Self(id, child))
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PageQuery {
    limit: Option<u32>,
    cursor: Option<String>,
}
pub(super) struct Page {
    limit: u32,
    cursor: Option<(String, DateTime<Utc>, Uuid)>,
}
impl Page {
    pub fn for_kind(self, kind: ContestKind) -> Result<ContestPageRequest, ContestError> {
        let after = match self.cursor {
            Some((scope, time, id)) => {
                if scope != kind.as_str() {
                    return Err(invalid_cursor());
                }
                Some((time, id))
            }
            None => None,
        };
        Ok(ContestPageRequest {
            limit: self.limit,
            after,
        })
    }
}
impl<S: Send + Sync> FromRequestParts<S> for Page {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Query(query) = Query::<PageQuery>::from_request_parts(parts, state)
            .await
            .map_err(|_| ApiError::BadRequest)?;
        let cursor = if let Some(value) = query.cursor {
            if value.is_empty() || value.len() > 2048 {
                return Err(invalid_cursor().into());
            }
            let decoded = URL_SAFE_NO_PAD
                .decode(value)
                .map_err(|_| invalid_cursor())?;
            let decoded = String::from_utf8(decoded).map_err(|_| invalid_cursor())?;
            let parts = decoded.split('|').collect::<Vec<_>>();
            let [kind, time, id] = parts.as_slice() else {
                return Err(invalid_cursor().into());
            };
            Some((
                kind.to_string(),
                DateTime::parse_from_rfc3339(time)
                    .map_err(|_| invalid_cursor())?
                    .to_utc(),
                id.parse().map_err(|_| invalid_cursor())?,
            ))
        } else {
            None
        };
        Ok(Self {
            limit: query.limit.unwrap_or(30),
            cursor,
        })
    }
}
fn invalid_cursor() -> ContestError {
    ContestError::validation("cursor", "invalid cursor")
}
pub(super) fn encode_cursor(kind: ContestKind, (time, id): (DateTime<Utc>, Uuid)) -> String {
    URL_SAFE_NO_PAD.encode(format!("{}|{}|{}", kind.as_str(), time.to_rfc3339(), id))
}
