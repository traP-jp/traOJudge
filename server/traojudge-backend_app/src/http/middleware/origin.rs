use crate::http::error::ApiError;
use axum::{
    extract::{Request, State},
    http::{HeaderMap, Method, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::io;
use url::Url;
#[derive(Clone)]
pub struct WebOrigin(url::Origin);

impl WebOrigin {
    pub fn parse(value: &str) -> Result<Self, io::Error> {
        let url = Url::parse(value).map_err(io::Error::other)?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(io::Error::other(
                "WebUI origin must contain only scheme, host and port",
            ));
        }
        Ok(Self(url.origin()))
    }

    fn allows(&self, headers: &HeaderMap) -> bool {
        let mut origins = headers.get_all(header::ORIGIN).iter();
        let Some(origin) = origins.next() else {
            return false;
        };
        origins.next().is_none()
            && origin
                .to_str()
                .ok()
                .and_then(|value| Self::parse(value).ok())
                .is_some_and(|origin| origin.0 == self.0)
    }
}

pub async fn verify_origin(
    State(origin): State<WebOrigin>,
    request: Request,
    next: Next,
) -> Response {
    if !matches!(
        *request.method(),
        Method::GET | Method::HEAD | Method::OPTIONS
    ) && !origin.allows(request.headers())
    {
        return ApiError::Forbidden.into_response();
    }
    next.run(request).await
}
