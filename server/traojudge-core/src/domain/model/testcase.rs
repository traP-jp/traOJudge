use super::problem::ProblemId;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub struct TestcaseId(Uuid);

impl std::fmt::Display for TestcaseId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<TestcaseId> for Uuid {
    fn from(id: TestcaseId) -> Self {
        id.0
    }
}

impl From<Uuid> for TestcaseId {
    fn from(id: Uuid) -> Self {
        Self(id)
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
