use super::schema;
use axum::{
    Json,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use traojudge_core::domain::model::account::AccountError;
use traojudge_core::domain::model::editorial::EditorialError;
use uuid::Uuid;

pub enum ApiError {
    Application(AccountError),
    Editorial(EditorialError),
    BadRequest,
    Forbidden,
    PreconditionRequired,
}
impl From<AccountError> for ApiError {
    fn from(error: AccountError) -> Self {
        Self::Application(error)
    }
}
impl From<EditorialError> for ApiError {
    fn from(error: EditorialError) -> Self {
        Self::Editorial(error)
    }
}
impl ApiError {
    pub fn storage(error: impl std::error::Error + Send + Sync + 'static) -> Self {
        AccountError::storage(error).into()
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let request_id = Uuid::new_v4();
        let (status, code, message) = match &self {
            Self::BadRequest => (
                StatusCode::BAD_REQUEST,
                "bad_request",
                "invalid request format".to_owned(),
            ),
            Self::Forbidden => (
                StatusCode::FORBIDDEN,
                "forbidden",
                "origin is not allowed".to_owned(),
            ),
            Self::PreconditionRequired => (
                StatusCode::PRECONDITION_REQUIRED,
                "precondition_required",
                "If-Match is required".to_owned(),
            ),
            Self::Editorial(error) => {
                let (status, code) = match error {
                    EditorialError::Unauthenticated => {
                        (StatusCode::UNAUTHORIZED, "unauthenticated")
                    }
                    EditorialError::Forbidden => (StatusCode::FORBIDDEN, "forbidden"),
                    EditorialError::NotFound => (StatusCode::NOT_FOUND, "not_found"),
                    EditorialError::Conflict => (StatusCode::CONFLICT, "conflict"),
                    EditorialError::RevisionMismatch => {
                        (StatusCode::PRECONDITION_FAILED, "precondition_failed")
                    }
                    EditorialError::Validation { .. } => {
                        (StatusCode::UNPROCESSABLE_ENTITY, "validation_failed")
                    }
                    EditorialError::Storage(_) => {
                        tracing::error!(%request_id,"request storage operation failed");
                        (StatusCode::INTERNAL_SERVER_ERROR, "unexpected_error")
                    }
                };
                (status, code, error.to_string())
            }
            Self::Application(error) => {
                let (status, code) = match error {
                    AccountError::Unauthenticated => (StatusCode::UNAUTHORIZED, "unauthenticated"),
                    AccountError::NotFound => (StatusCode::NOT_FOUND, "not_found"),
                    AccountError::Conflict => (StatusCode::CONFLICT, "conflict"),
                    AccountError::RevisionMismatch => {
                        (StatusCode::PRECONDITION_FAILED, "precondition_failed")
                    }
                    AccountError::Validation { .. } => {
                        (StatusCode::UNPROCESSABLE_ENTITY, "validation_failed")
                    }
                    AccountError::Unavailable => {
                        (StatusCode::SERVICE_UNAVAILABLE, "service_unavailable")
                    }
                    AccountError::Storage(_) => {
                        tracing::error!(%request_id, "request storage operation failed");
                        (StatusCode::INTERNAL_SERVER_ERROR, "unexpected_error")
                    }
                };
                (status, code, error.to_string())
            }
        };
        let mut payload = schema::Error::new(code.to_owned(), message, request_id);
        let details = match self {
            Self::Application(AccountError::Validation { field, reason })
            | Self::Editorial(EditorialError::Validation { field, reason }) => {
                Some((field, reason))
            }
            _ => None,
        };
        if let Some((field, reason)) = details {
            payload.details = Some(vec![schema::ErrorDetail::new(
                field.to_owned(),
                reason.to_owned(),
            )]);
        }
        (status, [(header::CACHE_CONTROL, "no-store")], Json(payload)).into_response()
    }
}
