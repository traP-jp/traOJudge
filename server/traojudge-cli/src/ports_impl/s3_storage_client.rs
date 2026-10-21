use async_trait::async_trait;

use crate::{domain::file::UploadableFile, ports::storage_client::StorageClient};

pub struct S3StorageClient {
}

#[async_trait]
impl StorageClient for S3StorageClient {
    async fn upload_file(&self, file: UploadableFile) -> Result<(), anyhow::Error> {
        let content = tokio::fs::read(&file.path).await?;
        let client = reqwest::Client::new();
        let result = client.put(file.url).body(content).send().await?;
        if result.status().is_success() {
            return Ok(())
        } else {
            let reason = result.status().canonical_reason();
            match reason {
                None => {
                    return Err(anyhow::Error::msg("Invalid status code was found."))
                }
                Some(expression) => {
                    return Err(anyhow::Error::msg(expression))
                }
            };
        } 
    }
}
