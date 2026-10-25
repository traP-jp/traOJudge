use crate::http::{error::ApiError, schema};
use axum::{
    Json,
    extract::{FromRequest, Request},
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use traojudge_core::domain::model::account::AccountError;

/// 生成型だけでは保証されないadditionalProperties:falseと非nullableをHTTP境界で検査する。
/// 対象はこのモジュールに登録した、文字列フィールドからなる要求型。
pub struct StrictJson<T>(pub T);
trait RequestSchema: DeserializeOwned {
    const FIELDS: &'static [&'static str];
}
impl RequestSchema for schema::ProfileUpdate {
    const FIELDS: &'static [&'static str] = &["displayName", "bio", "githubId", "atcoderId", "xId"];
}
impl RequestSchema for schema::DeveloperTokenRequest {
    const FIELDS: &'static [&'static str] = &["name", "expiresAt"];
}
impl RequestSchema for schema::EmailChangeRequest {
    const FIELDS: &'static [&'static str] = &["email"];
}
impl RequestSchema for schema::TokenConfirmation {
    const FIELDS: &'static [&'static str] = &["token"];
}
impl RequestSchema for schema::ExternalAuthorizationRequest {
    const FIELDS: &'static [&'static str] = &["provider"];
}
impl RequestSchema for schema::AuthorizationCompletion {
    const FIELDS: &'static [&'static str] = &["code", "state"];
}

impl RequestSchema for schema::EditorialCreate {
    const FIELDS: &'static [&'static str] = &["problemId", "title", "body"];
}
impl RequestSchema for schema::EditorialUpdate {
    const FIELDS: &'static [&'static str] = &["title", "body"];
}

impl<S, T> FromRequest<S> for StrictJson<T>
where
    S: Send + Sync,
    T: RequestSchema + Send,
{
    type Rejection = ApiError;
    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<Value>::from_request(request, state)
            .await
            .map_err(|_| ApiError::BadRequest)?;
        let object = value
            .as_object()
            .ok_or_else(|| AccountError::validation("body", "must be an object"))?;
        if object
            .iter()
            .any(|(name, value)| !T::FIELDS.contains(&name.as_str()) || !value.is_string())
        {
            return Err(AccountError::validation(
                "body",
                "unknown field, null, or non-string value",
            )
            .into());
        }
        serde_json::from_value(value).map(Self).map_err(|_| {
            AccountError::validation("body", "required field missing or invalid value").into()
        })
    }
}
