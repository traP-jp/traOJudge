use std::sync::Arc;

use crate::{
    infrastructure::{
        external::mail::MailClientImpl,
        provider::Provider,
        repository::{
            auth::AuthRepositoryImpl, problem::ProblemRepositoryImpl,
            session::SessionRepositoryImpl, submission::SubmissionRepositoryImpl,
            user::UserRepositoryImpl,
        },
    },
    usecase::service::{
        auth::AuthenticationService, submission::SubmissionService, user::UserService,
    },
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
    submission_service: Arc<
        SubmissionService<SessionRepositoryImpl, SubmissionRepositoryImpl, ProblemRepositoryImpl>,
    >,
}

impl DiContainer {
    pub async fn new(provider: Provider) -> Self {
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
            submission_service: Arc::new(SubmissionService::new(
                provider.provide_session_repository(),
                provider.provide_submission_repository(),
                provider.provide_problem_repository(),
            )),
        }
    }

    pub fn user_service(
        &self,
    ) -> &UserService<UserRepositoryImpl, SessionRepositoryImpl, AuthRepositoryImpl, MailClientImpl>
    {
        &self.user_service
    }

    pub fn auth_service(
        &self,
    ) -> &AuthenticationService<
        AuthRepositoryImpl,
        UserRepositoryImpl,
        SessionRepositoryImpl,
        MailClientImpl,
    > {
        &self.auth_service
    }

    pub fn submission_service(
        &self,
    ) -> &SubmissionService<SessionRepositoryImpl, SubmissionRepositoryImpl, ProblemRepositoryImpl>
    {
        &self.submission_service
    }
}
