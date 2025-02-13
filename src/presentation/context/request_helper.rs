use axum::{
    async_trait,
    extract::{rejection::JsonRejection, FromRequest, Request},
    Json,
};
use reqwest::StatusCode;
use serde::de::DeserializeOwned;

use super::validate::{ValidatedRequest, Validator};

#[async_trait]
impl<T, S> FromRequest<S> for ValidatedRequest<T>
where
    T: DeserializeOwned + Validator,
    S: Send + Sync,
    Json<T>: FromRequest<S, Rejection = JsonRejection>,
{
    type Rejection = StatusCode;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(req, state)
            .await
            .map_err(|_| StatusCode::BAD_REQUEST)?;
        value.validate().map_err(|_| StatusCode::BAD_REQUEST)?;
        Ok(ValidatedRequest(value))
    }
}
