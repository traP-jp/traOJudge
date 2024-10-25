use super::Repository;
use async_session::SessionStore;

impl Repository {
    pub async fn get_display_id_by_session_id(
        &self,
        session_id: &str,
    ) -> anyhow::Result<Option<i64>> {
        let session = self
            .session_store
            .load_session(session_id.to_owned())
            .await?;

        Ok(session.and_then(|s| s.get("display_id")))
    }
}
