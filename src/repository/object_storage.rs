use aws_sdk_s3::primitives::ByteStream;

use crate::Repository;

impl Repository {
    pub async fn upload_object(&self, key: &str, data: &str) -> anyhow::Result<()> {
        self.s3_client
            .put_object()
            .bucket(self.bucket_name.to_string())
            .key(key.to_string())
            .body(ByteStream::from(data.as_bytes().to_vec()))
            .send()
            .await?;

        Ok(())
    }

    pub async fn get_object(&self, key: &str) -> anyhow::Result<String> {
        let res = self
            .s3_client
            .get_object()
            .bucket(self.bucket_name.to_string())
            .key(key.to_string())
            .send()
            .await?;

        let body = res.body.collect().await?;
        let body = String::from_utf8(body.to_vec())?;

        Ok(body)
    }
}
