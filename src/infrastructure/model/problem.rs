use sqlx::types::chrono;

use crate::domain::model::problem::NormalProblem;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct NormalProblemPow {
    pub id: i64,
    pub author_id: i64,
    pub title: String,
    pub statement: String,
    pub time_limit: i32,
    pub memory_limit: i32,
    pub difficulty: i32,
    pub is_public: bool,
    pub judgecode_path: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<NormalProblemPow> for NormalProblem {
    fn from(val: NormalProblemPow) -> Self {
        NormalProblem {
            id: val.id,
            author_id: val.author_id,
            title: val.title,
            statement: val.statement,
            time_limit: val.time_limit,
            memory_limit: val.memory_limit,
            difficulty: val.difficulty,
            is_public: val.is_public,
            judgecode_path: val.judgecode_path,
            created_at: val.created_at,
            updated_at: val.updated_at,
        }
    }
}
