use super::user::UserId;
use anyhow::Context;
use chrono::{DateTime, Utc};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProblemId(i64);

impl FromStr for ProblemId {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = s.parse().context("failed to parse ProblemId from str")?;
        Ok(ProblemId(id))
    }
}

impl From<i64> for ProblemId {
    fn from(id: i64) -> Self {
        Self(id)
    }
}

impl From<ProblemId> for i64 {
    fn from(id: ProblemId) -> Self {
        id.0
    }
}

#[derive(Debug, Clone)]
pub struct Problem {
    pub id: ProblemId,
    pub author_user_id: UserId,
    pub title: String,
    pub statement: String,
    pub time_limit_ms: i32,
    pub memory_limit_mib: i32,
    pub difficulty: i32,
    pub is_public: bool,
    pub solved_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct UpdateProblem {
    pub title: String,
    pub is_public: bool,
    pub difficulty: i32,
    pub statement: String,
    pub time_limit_ms: i32,
    pub memory_limit_mib: i32,
}

pub struct CreateProblem {
    pub author_user_id: UserId,
    pub title: String,
    pub statement: String,
    pub time_limit_ms: i32,
    pub memory_limit_mib: i32,
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
    pub user_id: Option<UserId>,
    pub limit: i64,
    pub offset: i64,
    pub order_by: ProblemOrderBy,
    pub user_name: Option<String>,
    pub user_query: Option<UserId>,
}
