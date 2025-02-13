use di::DiContainer;

pub mod di;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
pub mod usecase;

pub fn make_router(app_state: DiContainer) -> axum::Router {
    
}
