use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use async_trait::async_trait;
use tokio::{
    fs::{self, File},
    io::{self, AsyncRead, AsyncWrite, AsyncWriteExt},
};
use traojudge_core::domain::{
    model::file::StoreFileId,
    service::file::{FileDownloader, FileStorage, FileUploader},
};
use uuid::Uuid;

pub fn new_local_file_store(
    store_root_path: &Path,
) -> Result<(LocalFileDownloader, LocalFileUploader, LocalFileStorage)> {
    std::fs::create_dir_all(store_root_path).with_context(|| {
        format!(
            "failed to create local file store root: {}",
            store_root_path.display()
        )
    })?;

    let store_root_path = store_root_path.to_path_buf();

    Ok((
        LocalFileDownloader {
            store_root_path: store_root_path.clone(),
        },
        LocalFileUploader {
            store_root_path: store_root_path.clone(),
        },
        LocalFileStorage { store_root_path },
    ))
}

#[derive(Debug, Clone)]
pub struct LocalFileUploader {
    store_root_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct LocalFileDownloader {
    store_root_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct LocalFileStorage {
    store_root_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct LocalFileUploadKey {
    path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct LocalFileDownloadKey {
    path: PathBuf,
}

#[async_trait]
impl FileUploader for LocalFileUploader {
    type UploadKey = LocalFileUploadKey;

    async fn upload_file<Readable: AsyncRead + Unpin + Send>(
        &self,
        from: &mut Readable,
        upload_key: Self::UploadKey,
    ) -> Result<()> {
        ensure_store_path(&self.store_root_path, &upload_key.path)?;

        let parent = upload_key
            .path
            .parent()
            .with_context(|| format!("upload path has no parent: {}", upload_key.path.display()))?;
        fs::create_dir_all(parent)
            .await
            .with_context(|| format!("failed to create upload directory: {}", parent.display()))?;

        let mut file = File::create(&upload_key.path).await.with_context(|| {
            format!(
                "failed to create upload file: {}",
                upload_key.path.display()
            )
        })?;
        io::copy(from, &mut file).await.with_context(|| {
            format!("failed to write upload file: {}", upload_key.path.display())
        })?;
        file.flush().await.with_context(|| {
            format!("failed to flush upload file: {}", upload_key.path.display())
        })?;

        Ok(())
    }
}

#[async_trait]
impl FileDownloader for LocalFileDownloader {
    type DownloadKey = LocalFileDownloadKey;

    async fn download_file<Writable: AsyncWrite + Unpin + Send>(
        &self,
        to: &mut Writable,
        download_key: Self::DownloadKey,
    ) -> Result<()> {
        ensure_store_path(&self.store_root_path, &download_key.path)?;

        let mut file = File::open(&download_key.path).await.with_context(|| {
            format!(
                "failed to open download file: {}",
                download_key.path.display()
            )
        })?;
        io::copy(&mut file, to).await.with_context(|| {
            format!(
                "failed to read download file: {}",
                download_key.path.display()
            )
        })?;
        to.flush()
            .await
            .context("failed to flush download output")?;

        Ok(())
    }
}

#[async_trait]
impl FileStorage for LocalFileStorage {
    type UploadKey = LocalFileUploadKey;
    type DownloadKey = LocalFileDownloadKey;

    async fn issue_upload_keys(
        &self,
        file_ids: &Vec<StoreFileId>,
    ) -> Result<Vec<(StoreFileId, Self::UploadKey)>> {
        Ok(file_ids
            .iter()
            .map(|file_id| {
                (
                    *file_id,
                    LocalFileUploadKey {
                        path: self.path_for_file_id(*file_id),
                    },
                )
            })
            .collect())
    }

    async fn issue_download_keys(
        &self,
        file_ids: &Vec<StoreFileId>,
    ) -> Result<Vec<(StoreFileId, Self::DownloadKey)>> {
        Ok(file_ids
            .iter()
            .map(|file_id| {
                (
                    *file_id,
                    LocalFileDownloadKey {
                        path: self.path_for_file_id(*file_id),
                    },
                )
            })
            .collect())
    }
}

impl LocalFileStorage {
    fn path_for_file_id(&self, file_id: StoreFileId) -> PathBuf {
        let file_id: Uuid = file_id.into();
        self.store_root_path.join(file_id.to_string())
    }
}

fn ensure_store_path(store_root_path: &Path, file_path: &Path) -> Result<()> {
    if !file_path.starts_with(store_root_path) {
        anyhow::bail!(
            "file path is outside local file store root: {}",
            file_path.display()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn uploads_and_downloads_file() -> Result<()> {
        let root_path = std::env::temp_dir().join(format!(
            "traojudge-local-file-store-test-{}",
            Uuid::new_v4()
        ));
        let (downloader, uploader, storage) = new_local_file_store(&root_path)?;
        let file_id = StoreFileId::from(Uuid::new_v4());
        let file_ids = vec![file_id];

        let mut upload_keys = storage.issue_upload_keys(&file_ids).await?;
        let (_, upload_key) = upload_keys.pop().context("upload key was not issued")?;
        let mut input: &[u8] = b"hello local file store";
        uploader.upload_file(&mut input, upload_key).await?;

        let mut download_keys = storage.issue_download_keys(&file_ids).await?;
        let (_, download_key) = download_keys.pop().context("download key was not issued")?;
        let mut output = Vec::new();
        downloader.download_file(&mut output, download_key).await?;

        assert_eq!(output, b"hello local file store");

        std::fs::remove_dir_all(root_path).ok();
        Ok(())
    }
}
