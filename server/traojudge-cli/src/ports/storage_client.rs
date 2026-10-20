use anyhow::Result;
use async_trait::async_trait;

use crate::domain::file::UploadableFile;

#[async_trait]
pub trait StorageClient {
    async fn upload_files(&self, files: Vec<UploadableFile>) -> Result<()>;
}
