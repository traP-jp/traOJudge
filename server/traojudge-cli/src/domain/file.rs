use std::path::PathBuf;

/// Abstraction for a pair of a file and presigned URL for uploading to a storage service.ss
pub struct UploadableFile {
    pub path: PathBuf,
    pub url: String,
}
