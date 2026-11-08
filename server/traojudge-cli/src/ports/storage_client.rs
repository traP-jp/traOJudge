use anyhow::Result;
use async_trait::async_trait;

use crate::domain::file::UploadableFile;

#[async_trait]
pub trait StorageClient {
    async fn upload_files(&self, files: Vec<UploadableFile>) -> Result<()> {
        futures::future::join_all(files.into_iter().map(|f| {
            self.upload_file(f)
        }))
            .await
            .into_iter()
            .collect::<Result<Vec<()>, _>>()
            .map(|_| ())
    }
    async fn upload_file(&self, file: UploadableFile) -> Result<()>;
}
