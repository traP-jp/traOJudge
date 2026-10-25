use crate::{
    domain::model::account::{AccountError, AuthenticatedSession, Secret},
    ports::repository::account::SessionRepository,
};

pub struct SessionService<R> {
    repository: R,
}

impl<R: SessionRepository> SessionService<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }
    pub async fn authenticate(
        &self,
        secret: &Secret,
    ) -> Result<Option<AuthenticatedSession>, AccountError> {
        self.repository.refresh_session(&secret.digest()).await
    }
}
