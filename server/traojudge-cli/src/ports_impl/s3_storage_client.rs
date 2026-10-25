use async_trait::async_trait;

use crate::{domain::file::UploadableFile, ports::storage_client::StorageClient};

pub struct S3StorageClient {
    pub client: reqwest::Client,
}

impl S3StorageClient {
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl StorageClient for S3StorageClient {
    async fn upload_file(&self, file: UploadableFile) -> Result<(), anyhow::Error> {
        let content = tokio::fs::read(&file.path).await?;
        let result = self
            .client
            .put(file.url)
            .header("x-amz-checksum-sha256", format!("{:x}", file.hash))
            .body(content)
            .send()
            .await?;
        if result.status().is_success() {
            Ok(())
        } else {
            let reason = result.status().canonical_reason();
            match reason {
                None => Err(anyhow::Error::msg("Invalid status code was found.")),
                Some(expression) => Err(anyhow::Error::msg(expression)),
            }
        }
    }
}
