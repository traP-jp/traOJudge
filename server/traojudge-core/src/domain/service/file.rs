use anyhow::Result;
use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::domain::{
    model::file::{StoreFileId, StoreFileMeta, StoreFileReferenceId, StoreFileStatus},
    service::unit_of_work_provider::UnitOfWorkProvider,
};

#[async_trait]
pub trait FileUploader {
    type UploadKey;
    async fn upload_file<Readable: AsyncRead>(
        &self,
        from: &Readable,
        upload_key: Self::UploadKey,
    ) -> Result<()>;
}

#[async_trait]
pub trait FileDownloader {
    type DownloadKey;
    async fn download_file<Writable: AsyncWrite>(
        &self,
        to: &Writable,
        download_key: Self::DownloadKey,
    ) -> Result<()>;
}

#[async_trait]
pub trait FileStorage {
    type UploadKey;
    type DownloadKey;

    /// Every element in `file_ids` must be unique.
    async fn issue_upload_keys(
        &self,
        file_ids: &Vec<StoreFileId>,
    ) -> Result<Vec<(StoreFileId, Self::UploadKey)>>;
    /// Some elements in `file_ids` may duplicate.
    async fn issue_download_keys(
        &self,
        file_ids: &Vec<StoreFileId>,
    ) -> Result<Vec<(StoreFileId, Self::DownloadKey)>>;
}

#[async_trait]
pub trait FileMetaRepository<UoWP: UnitOfWorkProvider> {
    async fn create_file_metas(
        &self,
        uow: &UoWP::UnitOfWork,
        count: usize,
    ) -> Result<Vec<StoreFileMeta>>;
    async fn read_file_metas(
        &self,
        uow: &UoWP::UnitOfWork,
        file_ids: &Vec<StoreFileId>,
    ) -> Result<Vec<StoreFileMeta>>;
    async fn update_file_status(
        &self,
        uow: &UoWP::UnitOfWork,
        file_id: StoreFileId,
        new_status: StoreFileStatus,
    ) -> Result<()>;
    async fn increment_file_ref(
        &self,
        uow: &UoWP::UnitOfWork,
        file_id: StoreFileId,
    ) -> Result<StoreFileReferenceId>;
    async fn decrement_file_ref(
        &self,
        uow: &UoWP::UnitOfWork,
        reference_id: StoreFileReferenceId,
    ) -> Result<usize>;
}
