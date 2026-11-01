use crate::http::{error::ApiError, schema};
use axum::{
    Json,
    extract::{FromRequest, Request},
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use traojudge_core::domain::model::account::AccountError;

/// 生成型で保持されないadditionalProperties:falseと非nullableを再帰的に検査する。
pub struct StrictJson<T>(pub T);
#[derive(Clone, Copy)]
enum Shape {
    String,
    Integer,
    Object(&'static [(&'static str, Shape)]),
}
trait RequestSchema: DeserializeOwned {
    const FIELDS: &'static [(&'static str, Shape)];
}
macro_rules! string_request {
    ($ty:ty; $($field:literal),* $(,)?) => {
        impl RequestSchema for $ty { const FIELDS: &'static [(&'static str,Shape)] = &[$(($field,Shape::String)),*]; }
    };
}
string_request!(schema::ProfileUpdate; "displayName","bio","githubId","atcoderId","xId");
string_request!(schema::DeveloperTokenRequest; "name","expiresAt");
string_request!(schema::EmailChangeRequest; "email");
string_request!(schema::TokenConfirmation; "token");
string_request!(schema::ExternalAuthorizationRequest; "provider");
string_request!(schema::AuthorizationCompletion; "code","state");
string_request!(schema::EditorialCreate; "problemId","title","body");
string_request!(schema::EditorialUpdate; "title","body");
string_request!(schema::ContestTeamJoinRequest; "name","password");
const RULES: Shape = Shape::Object(&[
    ("tieBreak", Shape::String),
    ("penaltyMinutes", Shape::Integer),
]);
impl RequestSchema for schema::ContestCreate {
    const FIELDS: &'static [(&'static str, Shape)] = &[
        ("title", Shape::String),
        ("description", Shape::String),
        ("participationMode", Shape::String),
        ("startsAt", Shape::String),
        ("endsAt", Shape::String),
        ("rules", RULES),
        ("visibility", Shape::String),
    ];
}
impl RequestSchema for schema::ContestUpdate {
    const FIELDS: &'static [(&'static str, Shape)] = &[
        ("title", Shape::String),
        ("visibility", Shape::String),
        ("description", Shape::String),
        ("participationMode", Shape::String),
        ("startsAt", Shape::String),
        ("endsAt", Shape::String),
        ("rules", RULES),
    ];
}
impl RequestSchema for schema::VirtualContestCreate {
    const FIELDS: &'static [(&'static str, Shape)] = &[
        ("title", Shape::String),
        ("startsAt", Shape::String),
        ("endsAt", Shape::String),
        ("rules", RULES),
        ("visibility", Shape::String),
    ];
}
impl RequestSchema for schema::VirtualContestUpdate {
    const FIELDS: &'static [(&'static str, Shape)] = &[
        ("title", Shape::String),
        ("visibility", Shape::String),
        ("startsAt", Shape::String),
        ("endsAt", Shape::String),
        ("rules", RULES),
    ];
}
impl RequestSchema for schema::ContestProblemInput {
    const FIELDS: &'static [(&'static str, Shape)] =
        &[("label", Shape::String), ("order", Shape::Integer)];
}
impl Shape {
    fn accepts(self, value: &Value) -> bool {
        match self {
            Self::String => value.is_string(),
            Self::Integer => value.is_i64() || value.is_u64(),
            Self::Object(fields) => value.as_object().is_some_and(|object| {
                object.iter().all(|(key, value)| {
                    fields
                        .iter()
                        .find(|(name, _)| key == name)
                        .is_some_and(|(_, shape)| shape.accepts(value))
                })
            }),
        }
    }
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
        if !Shape::Object(T::FIELDS).accepts(&value) {
            return Err(AccountError::validation(
                "body",
                "unknown field, null, or incorrect value type",
            )
            .into());
        }
        serde_json::from_value(value).map(Self).map_err(|_| {
            AccountError::validation("body", "required field missing or invalid value").into()
        })
    }
}
