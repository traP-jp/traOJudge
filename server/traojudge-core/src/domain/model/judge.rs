/*
/use crate::domain::model::{generic_dag::GenericDagId, submission::SubmissionId};

use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JudgeId(Uuid);

impl From<Uuid> for JudgeId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl Into<Uuid> for JudgeId {
    fn into(self) -> Uuid {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct Judge {
    pub id: JudgeId,
    pub definition_id: GenericDagId,
    pub submission_id: SubmissionId,
    pub language_id: String,
    pub status: JudgeStatus,
    pub result: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

*/
