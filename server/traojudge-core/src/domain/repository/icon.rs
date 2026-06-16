use anyhow::Result;
use async_trait::async_trait;

use crate::domain::model::icon::{CreateIcon, Icon, IconId};

#[async_trait]
pub trait IconRepository: Send {
    async fn get_icon(&mut self, id: IconId) -> Result<Option<Icon>>;
    async fn create_icon(&mut self, icon: CreateIcon) -> Result<IconId>;
    async fn delete_icon(&mut self, id: IconId) -> Result<()>;
}
