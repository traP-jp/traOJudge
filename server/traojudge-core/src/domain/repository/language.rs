use anyhow::Result;
use async_trait::async_trait;

use crate::domain::model::language::{Language, LanguageId};

#[async_trait]
pub trait LanguageRepository: Send {
    async fn get_languages(&mut self) -> Result<Vec<Language>>;
    async fn language_to_id(&mut self, language: &str) -> Result<Option<LanguageId>>;
    async fn id_to_language(&mut self, id: LanguageId) -> Result<Option<String>>;
}
