use anyhow::Result;
use async_trait::async_trait;

use crate::domain::model::editorial::{
    CreateEditorial, Editorial, EditorialGetQuery, EditorialId, EditorialSummary, UpdateEditorial,
};

#[async_trait]
pub trait EditorialRepository: Send {
    async fn get_editorial(&mut self, id: EditorialId) -> Result<Option<Editorial>>;
    async fn get_editorials_by_query(
        &mut self,
        query: EditorialGetQuery,
    ) -> Result<Vec<EditorialSummary>>;
    async fn create_editorial(&mut self, editorial: CreateEditorial) -> Result<EditorialId>;
    async fn update_editorial(&mut self, editorial: UpdateEditorial) -> Result<()>;
    async fn delete_editorial(&mut self, id: EditorialId) -> Result<()>;
}
