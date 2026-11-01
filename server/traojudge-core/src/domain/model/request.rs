/// 作成・参加要求の再送を識別するキー。HTTPや対象機能の型には依存しない。
pub struct IdempotencyKey(String);
#[derive(Debug, thiserror::Error)]
#[error("idempotency key must contain 1 to 128 ASCII letters, digits, '.', '_', ':' or '-'")]
pub struct InvalidIdempotencyKey;
impl IdempotencyKey {
    pub fn parse(value: &str) -> Result<Self, InvalidIdempotencyKey> {
        if !(1..=128).contains(&value.len())
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'-'))
        {
            return Err(InvalidIdempotencyKey);
        }
        Ok(Self(value.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ReceiptCleanupError {
    #[error("receipt cleanup batch size must be between 1 and 10000")]
    InvalidBatchSize,
    #[error("receipt cleanup persistence operation failed")]
    Storage(#[source] Box<dyn std::error::Error + Send + Sync>),
}
impl ReceiptCleanupError {
    pub fn storage(error: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Storage(Box::new(error))
    }
}

#[derive(Clone, Copy)]
pub struct ReceiptCleanupBatchSize(u32);
impl ReceiptCleanupBatchSize {
    pub fn new(value: u32) -> Result<Self, ReceiptCleanupError> {
        if !(1..=10_000).contains(&value) {
            return Err(ReceiptCleanupError::InvalidBatchSize);
        }
        Ok(Self(value))
    }
    pub fn get(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ReceiptCleanupCounts {
    pub editorial_receipts: u64,
    pub contest_receipts: u64,
}
