use super::problem::ProblemId;
use super::testcase::TestcaseId;
use super::user::UserId;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub struct SubmissionId(pub(crate) Uuid);

impl From<Uuid> for SubmissionId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl Into<Uuid> for SubmissionId {
    fn into(self) -> Uuid {
        self.0
    }
}

pub struct Submission {
    pub id: SubmissionId,
    pub user_id: UserId,
    pub user_name: String,
    pub problem_id: ProblemId,
    pub problem_title: String,
    pub submitted_at: DateTime<Utc>,
    pub language_id: String,
    pub total_score: i64,
    pub max_time_ms: i32,
    pub max_memory_kib: i32,
    pub source: String,
    pub overall_judge_status: String,
}

pub struct JudgeResult {
    pub testcase_id: TestcaseId,
    pub testcase_name: String,
    pub judge_status: String,
    pub score: i64,
    pub time_ms: i32,
    pub memory_kib: i32,
}

pub struct CreateSubmission {
    pub problem_id: ProblemId,
    pub user_id: UserId,
    pub language_id: String,
    pub source: String,
    pub judge_status: String,
    pub total_score: i64,
    pub max_time_ms: i32,
    pub max_memory_kib: i32,
}

pub struct UpdateSubmission {
    pub judge_status: String,
    pub total_score: i64,
    pub max_time_ms: i32,
    pub max_memory_kib: i32,
}

pub struct CreateJudgeResult {
    pub submission_id: SubmissionId,
    pub testcase_id: TestcaseId,
    pub testcase_name: String,
    pub judge_status: String,
    pub score: i64,
    pub time_ms: i32,
    pub memory_kib: i32,
}

#[derive(Clone)]
pub enum SubmissionOrderBy {
    SubmittedAtAsc,
    SubmittedAtDesc,
    TimeConsumptionAsc,
    TimeConsumptionDesc,
    ScoreAsc,
    ScoreDesc,
    MemoryConsumptionAsc,
    MemoryConsumptionDesc,
    CodeLengthAsc,
    CodeLengthDesc,
}

#[derive(Clone)]
pub struct SubmissionGetQuery {
    pub user_id: Option<UserId>,
    pub limit: i64,
    pub offset: i64,
    pub judge_status: Option<String>,
    pub language_id: Option<String>,
    pub user_name: Option<String>,
    pub user_query: Option<UserId>,
    pub order_by: SubmissionOrderBy,
    pub problem_id: Option<ProblemId>,
}
