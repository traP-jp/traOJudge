use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::user::UserId;

#[derive(Debug, thiserror::Error)]
pub enum EditorialError {
    #[error("authentication required")]
    Unauthenticated,
    #[error("operation is not permitted")]
    Forbidden,
    #[error("resource not found")]
    NotFound,
    #[error("the idempotency key was already used with different input")]
    Conflict,
    #[error("resource revision changed")]
    RevisionMismatch,
    #[error("invalid {field}: {reason}")]
    Validation {
        field: &'static str,
        reason: &'static str,
    },
    #[error("editorial storage operation failed")]
    Storage(#[source] Box<dyn std::error::Error + Send + Sync>),
}
impl EditorialError {
    pub fn storage(error: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Storage(Box::new(error))
    }
    pub fn validation(field: &'static str, reason: &'static str) -> Self {
        Self::Validation { field, reason }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorialVisibility {
    Public,
    Private,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Editorial {
    pub id: Uuid,
    pub problem_id: Uuid,
    pub author_id: UserId,
    pub title: String,
    pub body: String,
    pub visibility: EditorialVisibility,
    pub revision: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct NewEditorial {
    pub problem_id: Uuid,
    pub title: String,
    pub body: String,
}
impl NewEditorial {
    pub fn validate(&self) -> Result<(), EditorialError> {
        validate_text("title", &self.title, 200)?;
        validate_text("body", &self.body, 200_000)
    }
    /// JSONのキー順に依存せず、各項目の境界を保持して同じ入力を識別する。
    pub fn digest(&self) -> Vec<u8> {
        let mut digest = Sha256::new();
        digest.update(self.problem_id.as_bytes());
        for value in [&self.title, &self.body] {
            digest.update((value.len() as u64).to_be_bytes());
            digest.update(value.as_bytes());
        }
        digest.finalize().to_vec()
    }
}

#[derive(Default)]
pub struct EditorialChanges {
    pub title: Option<String>,
    pub body: Option<String>,
}
impl EditorialChanges {
    pub fn validate(&self) -> Result<(), EditorialError> {
        if self.title.is_none() && self.body.is_none() {
            return Err(EditorialError::validation(
                "body",
                "at least one change is required",
            ));
        }
        if let Some(title) = &self.title {
            validate_text("title", title, 200)?;
        }
        if let Some(body) = &self.body {
            validate_text("body", body, 200_000)?;
        }
        Ok(())
    }
    pub fn apply(self, editorial: &mut Editorial) {
        if let Some(title) = self.title {
            editorial.title = title;
        }
        if let Some(body) = self.body {
            editorial.body = body;
        }
    }
}
fn validate_text(field: &'static str, value: &str, max: usize) -> Result<(), EditorialError> {
    if value.is_empty() || value.chars().count() > max || value.contains('\0') {
        return Err(EditorialError::validation(
            field,
            "must be nonempty, within the character limit, and contain no NUL",
        ));
    }
    Ok(())
}

pub struct EditorialCreationKey(String);
impl EditorialCreationKey {
    pub fn parse(value: &str) -> Result<Self, EditorialError> {
        if !(1..=128).contains(&value.len())
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'-'))
        {
            return Err(EditorialError::validation(
                "Idempotency-Key",
                "must contain 1 to 128 ASCII letters, digits, '.', '_', ':' or '-'",
            ));
        }
        Ok(Self(value.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub struct EditorialCreationReceipt {
    pub request_digest: Vec<u8>,
    pub editorial: Editorial,
}

#[derive(Clone, Debug)]
pub struct EditorialCursor {
    pub problem_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub id: Uuid,
}
pub struct EditorialPageRequest {
    pub limit: u32,
    pub cursor: Option<EditorialCursor>,
}
impl EditorialPageRequest {
    pub fn validate(&self, problem_id: Uuid) -> Result<(), EditorialError> {
        if !(1..=100).contains(&self.limit) {
            return Err(EditorialError::validation(
                "limit",
                "must be between 1 and 100",
            ));
        }
        if self
            .cursor
            .as_ref()
            .is_some_and(|cursor| cursor.problem_id != problem_id)
        {
            return Err(EditorialError::validation(
                "cursor",
                "cursor belongs to another problem",
            ));
        }
        Ok(())
    }
}
pub struct EditorialPage {
    pub items: Vec<Editorial>,
    pub next_cursor: Option<EditorialCursor>,
}
