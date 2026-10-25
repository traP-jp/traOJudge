use crate::http::error::ApiError;
use axum::{
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;
use traojudge_core::{
    application::session::SessionService, domain::model::account::Secret,
    ports::repository::account::SessionRepository,
};
const SESSION_COOKIE: &str = "traojudge_session";
fn session_secret(headers: &HeaderMap) -> Option<Secret> {
    let mut value = None;
    for header in headers.get_all(header::COOKIE) {
        for cookie in header.to_str().ok()?.split(';') {
            let Some((name, candidate)) = cookie.trim().split_once('=') else {
                continue;
            };
            if name == SESSION_COOKIE {
                if value.is_some() {
                    return None;
                }
                value = Some(candidate);
            }
        }
    }
    Secret::parse(value?).ok()
}

pub async fn authenticate<R: SessionRepository>(
    State(service): State<Arc<SessionService<R>>>,
    mut request: Request,
    next: Next,
) -> Response {
    let secret = session_secret(request.headers());
    let actor = match &secret {
        Some(secret) => match service.authenticate(secret).await {
            Ok(actor) => actor,
            Err(error) => return ApiError::from(error).into_response(),
        },
        None => None,
    };
    if let Some(actor) = actor {
        request.extensions_mut().insert(actor);
    }
    let mut response = next.run(request).await;
    if let (Some(_), Some(secret)) = (actor, secret) {
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        let cookie = format!(
            "{SESSION_COOKIE}={}; Path=/; HttpOnly; Secure; SameSite=Lax; Max-Age=7776000",
            secret.expose()
        );
        // Secretの文字種検証によりCookieヘッダーへ安全に埋め込める。
        if let Ok(cookie) = HeaderValue::from_str(&cookie) {
            response.headers_mut().append(header::SET_COOKIE, cookie);
        }
    }
    response
}
