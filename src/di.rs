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
    auth_service: AuthenticationService<
        AuthRepositoryImpl,
        UserRepositoryImpl,
        SessionRepositoryImpl,
        MailClientImpl,
    >,
    user_service: UserService<UserRepositoryImpl>,
}

impl DiContainer {
    async fn new() -> Self {
        let provider = Provider::new().await.unwrap();

        Self {
            auth_service: AuthenticationService::new(
                provider.provide_auth_repository(),
                provider.provide_user_repository(),
                provider.provide_session_repository(),
                provider.provide_mail_client(),
            ),
            user_service: UserService::new(provider.provide_user_repository()),
        }
    }

    fn user_service(&self) -> &UserService<UserRepositoryImpl> {
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
