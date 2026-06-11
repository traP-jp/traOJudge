use super::{problem::ProblemId, user::UserId};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EditorialId(Uuid);

impl From<Uuid> for EditorialId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl Into<Uuid> for EditorialId {
    fn into(self) -> Uuid {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct Editorial {
    pub id: EditorialId,
    pub problem_id: ProblemId,
    pub author_id: UserId,
    pub title: String,
    pub statement: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub is_public: bool,
}

#[derive(Debug, Clone)]
pub struct CreateEditorial {
    pub problem_id: ProblemId,
    pub author_id: UserId,
    pub title: String,
    pub statement: String,
    pub is_public: bool,
}

#[derive(Debug, Clone)]
pub struct UpdateEditorial {
    pub id: EditorialId,
    pub title: String,
    pub statement: String,
    pub is_public: bool,
}

#[derive(Debug, Clone)]
pub struct EditorialSummary {
    pub id: EditorialId,
    pub problem_id: ProblemId,
    pub author_id: UserId,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub is_public: bool,
}

#[derive(Debug, Clone)]
pub struct EditorialGetQuery {
    pub user_id: Option<UserId>,
    pub problem_id: ProblemId,
    pub limit: i64,
    pub offset: i64,
}
