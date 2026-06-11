use super::problem::ProblemId;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub struct TestcaseId(Uuid);

impl From<Uuid> for TestcaseId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl Into<Uuid> for TestcaseId {
    fn into(self) -> Uuid {
        self.0
    }
}

impl Default for TestcaseId {
    fn default() -> Self {
        Self(Uuid::new_v4())
    }
}

pub struct TestcaseSummary {
    pub id: TestcaseId,
    pub name: String,
    pub problem_id: ProblemId,
    pub input_id: Uuid,
    pub output_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct CreateTestcase {
    pub id: TestcaseId,
    pub name: String,
    pub problem_id: ProblemId,
    pub input_id: Uuid,
    pub output_id: Uuid,
}
