use uuid::Uuid;

/// [`FileId`] is the unique identifier of a stored file with 1-to-1 correspondence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StoreFileId(Uuid);

impl From<Uuid> for StoreFileId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl Into<Uuid> for StoreFileId {
    fn into(self) -> Uuid {
        self.0
    }
}

/// [`FileReferenceId`]s are shared pointers to a [`FileId`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StoreFileReferenceId(Uuid);

impl From<Uuid> for StoreFileReferenceId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl Into<Uuid> for StoreFileReferenceId {
    fn into(self) -> Uuid {
        self.0
    }
}

#[derive(Debug, Clone)]
pub enum StoreFileStatus {
    Uploading,
    Uploaded,
    UploadFailed,
}

#[derive(Debug, Clone)]
pub struct StoreFileMeta {
    pub id: StoreFileId,
    pub status: StoreFileStatus,
}
