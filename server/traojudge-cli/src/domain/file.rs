use std::path::PathBuf;

use sha2::{digest::Output, Sha256};

/// Abstraction for a pair of a file and presigned URL for uploading to a storage service.ss
pub struct UploadableFile {
    pub path: PathBuf,
    pub url: String,
    pub hash: Output<Sha256>,
}
