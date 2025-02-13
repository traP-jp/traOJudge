use async_sqlx_session::MySqlSessionStore;
use sqlx::MySqlPool;

use super::{
    external::mail::MailClientImpl,
    repository::{
        auth::AuthRepositoryImpl, session::SessionRepositoryImpl, user::UserRepositoryImpl,
    },
};

pub struct InfraModules {
    mail_client: MailClientImpl,
    auth_repository: AuthRepositoryImpl,
    user_repository: UserRepositoryImpl,
    session_repository: SessionRepositoryImpl,
}

pub trait InfraModulesExt {
    fn mail_client(&self) -> &MailClientImpl;
    fn auth_repository(&self) -> &AuthRepositoryImpl;
    fn user_repository(&self) -> &UserRepositoryImpl;
    fn session_repository(&self) -> &SessionRepositoryImpl;
}

impl InfraModulesExt for InfraModules {
    fn mail_client(&self) -> &MailClientImpl {
        &self.mail_client
    }

    fn auth_repository(&self) -> &AuthRepositoryImpl {
        &self.auth_repository
    }

    fn user_repository(&self) -> &UserRepositoryImpl {
        &self.user_repository
    }

    fn session_repository(&self) -> &SessionRepositoryImpl {
        &self.session_repository
    }
}

impl InfraModules {
    pub fn new(db: MySqlPool, session_store: MySqlSessionStore) -> anyhow::Result<Self> {
        let mail_client = MailClientImpl::new()?;
        let auth_repository = AuthRepositoryImpl::new(12, db.clone());
        let user_repository = UserRepositoryImpl::new(db);
        let session_repository = SessionRepositoryImpl::new(session_store);

        Ok(Self {
            mail_client,
            auth_repository,
            user_repository,
            session_repository,
        })
    }
}
