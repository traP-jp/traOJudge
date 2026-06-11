use super::user::UserName;
use anyhow::Context;
use chrono::{DateTime, Utc};
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProblemId(Uuid);

impl FromStr for ProblemId {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = Uuid::parse_str(s).context("failed to parse ProblemId from str")?;
        Ok(ProblemId(id))
    }
}

impl From<Uuid> for ProblemId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl Into<Uuid> for ProblemId {
    fn into(self) -> Uuid {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct NormalProblem {
    pub id: ProblemId,
    pub author_name: UserName,
    pub title: String,
    pub statement: String,
    pub time_limit_ms: i32,
    pub memory_limit_kib: i32,
    pub difficulty: i32,
    pub is_public: bool,
    pub solved_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct UpdateNormalProblem {
    pub title: String,
    pub is_public: bool,
    pub difficulty: i32,
    pub statement: String,
    pub time_limit_ms: i32,
    pub memory_limit_kib: i32,
}

pub struct CreateNormalProblem {
    pub author_name: UserName,
    pub title: String,
    pub statement: String,
    pub time_limit_ms: i32,
    pub memory_limit_kib: i32,
    pub difficulty: i32,
}

#[derive(Clone)]
pub enum ProblemOrderBy {
    CreatedAtAsc,
    CreatedAtDesc,
    UpdatedAtAsc,
    UpdatedAtDesc,
    DifficultyAsc,
    DifficultyDesc,
}

#[derive(Clone)]
pub struct ProblemGetQuery {
    pub user_name: Option<UserName>,
    pub limit: i64,
    pub offset: i64,
    pub order_by: ProblemOrderBy,
    pub user_query: Option<UserName>,
}
