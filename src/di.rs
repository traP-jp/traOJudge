use std::sync::Arc;

use crate::{
    infrastructure::{
        external::mail::MailClientImpl,
        provider::Provider,
        repository::{
            auth::AuthRepositoryImpl, session::SessionRepositoryImpl, user::UserRepositoryImpl,
        },
    },
    usecase::service::{auth::AuthenticationService, user::UserService},
};

#[derive(Clone)]
pub struct DiContainer {
    auth_service: Arc<
        AuthenticationService<
            AuthRepositoryImpl,
            UserRepositoryImpl,
            SessionRepositoryImpl,
            MailClientImpl,
        >,
    >,
    user_service: Arc<
        UserService<UserRepositoryImpl, SessionRepositoryImpl, AuthRepositoryImpl, MailClientImpl>,
    >,
}

impl DiContainer {
    async fn new() -> Self {
        let provider = Provider::new().await.unwrap();

        Self {
            auth_service: Arc::new(AuthenticationService::new(
                provider.provide_auth_repository(),
                provider.provide_user_repository(),
                provider.provide_session_repository(),
                provider.provide_mail_client(),
            )),
            user_service: Arc::new(UserService::new(
                provider.provide_user_repository(),
                provider.provide_session_repository(),
                provider.provide_auth_repository(),
                provider.provide_mail_client(),
            )),
        }
    }

    fn user_service(
        &self,
    ) -> &UserService<UserRepositoryImpl, SessionRepositoryImpl, AuthRepositoryImpl, MailClientImpl>
    {
        &self.user_service
    }

    fn auth_service(
        &self,
    ) -> &AuthenticationService<
        AuthRepositoryImpl,
        UserRepositoryImpl,
        SessionRepositoryImpl,
        MailClientImpl,
    > {
        &self.auth_service
    }
}
