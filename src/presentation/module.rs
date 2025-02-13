use crate::domain::repository::{
    auth::AuthRepository, session::SessionRepository, user::UserRepository,
};

pub struct Modules {
    auth_reository: Box<dyn AuthRepository>,
    session_repository: Box<dyn SessionRepository>,
    user_repository: Box<dyn UserRepository>,
}

impl Modules {
    pub fn new(
        auth_reository: Box<dyn AuthRepository>,
        session_repository: Box<dyn SessionRepository>,
        user_repository: Box<dyn UserRepository>,
    ) -> Self {
        Self {
            auth_reository,
            session_repository,
            user_repository,
        }
    }

    pub fn auth_repository(&self) -> &Box<dyn AuthRepository> {
        &self.auth_reository
    }

    pub fn session_repository(&self) -> &Box<dyn SessionRepository> {
        &self.session_repository
    }

    pub fn user_repository(&self) -> &Box<dyn UserRepository> {
        &self.user_repository
    }
}
