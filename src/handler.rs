use axum::{
    routing::{get, post, put},
    Router,
};

use crate::repository::Repository;

mod authentication;
mod users;

pub fn make_router(app_state: Repository) -> Router {
    let authentication_router =
        Router::new().route("/signup/request", post(authentication::sign_up_request));
        
    let users_router = Router::new()
        .route("/me", get(users::get_me).put(users::put_me))
        .route("/me/email", put(users::put_me_email));

    Router::new()
        .nest("/", authentication_router)
        .nest("/users", users_router)
        .with_state(app_state)
}
