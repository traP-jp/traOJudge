use super::Repository;



impl Repository {
    pub async fn make_jwt_and_save(&self, email: &str) -> anyhow::Result<Option<String>> {
        Ok(Some(email.to_owned()))
    }
}