use crate::domain::{entities::user::User, repository::session::SessionRepository};

#[derive(Clone)]
pub struct SessionService<R: SessionRepository> {
    repository: R,
}

impl<R: SessionRepository> SessionService<R> {
    pub fn new(repository: R) -> Self {
        Self { repository }
    }
}

impl<R: SessionRepository> SessionService<R> {
    pub async fn create_session(&self, user: User) -> anyhow::Result<String> {
        self.repository.create_session(user).await
    }

    pub async fn delete_session(&self, session_id: &str) -> anyhow::Result<Option<()>> {
        self.repository.delete_session(session_id).await
    }

    pub async fn get_user_id_by_session_id(
        &self,
        session_id: &str,
    ) -> anyhow::Result<Option<String>> {
        self.repository.get_user_id_by_session_id(session_id).await
    }

    pub async fn get_display_id_by_session_id(
        &self,
        session_id: &str,
    ) -> anyhow::Result<Option<i64>> {
        self.repository
            .get_display_id_by_session_id(session_id)
            .await
    }
}
