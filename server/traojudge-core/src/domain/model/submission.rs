use super::{language::LanguageId, problem::ProblemId};
use super::{testcase::TestcaseId, user::UserId};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub struct SubmissionId(i64);

impl From<i64> for SubmissionId {
    fn from(id: i64) -> Self {
        Self(id)
    }
}

impl From<SubmissionId> for i64 {
    fn from(id: SubmissionId) -> Self {
        id.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub struct JudgeRunId(Uuid);

impl From<Uuid> for JudgeRunId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl From<JudgeRunId> for Uuid {
    fn from(id: JudgeRunId) -> Self {
        id.0
    }
}

/// TODO: use [`crate::domain::model::judge_state`]
pub struct Submission {
    pub id: SubmissionId,
    pub user_id: UserId,
    pub problem_id: ProblemId,
    pub language_id: LanguageId,
    pub current_judge_id: Option<JudgeRunId>,
    pub code_length_bytes: i32,
    /*
    pub overall_judge_status: JudgeStatus,
    pub judge_progress_step: i32,
    pub total_score: i32,
    pub max_time_ms: i32,
    pub max_memory_mib: i32,
    */
    pub submitted_at: DateTime<Utc>,
    pub judged_at: Option<DateTime<Utc>>,
}

pub struct SubmissionSource {
    pub submission_id: SubmissionId,
    pub source_bundle: Vec<u8>,
}

pub struct SubmissionSummary {
    pub id: SubmissionId,
    pub user_id: UserId,
    pub user_name: String,
    pub problem_id: ProblemId,
    pub problem_title: String,
    pub submitted_at: DateTime<Utc>,
    pub language_id: LanguageId,
    pub current_judge_id: Option<JudgeRunId>,
    pub code_length_bytes: i32,
    /// TODO: use [`crate::domain::model::judge_state`]
    /*
    pub overall_judge_status: JudgeStatus,
    pub judge_progress_step: i32,
    pub total_score: i32,
    pub max_time_ms: i32,
    */
    pub max_memory_mib: i32,
    pub judged_at: Option<DateTime<Utc>>,
}

/// TODO: use [`crate::domain::model::judge_state`]
pub struct SubmissionJudgeRun {
    pub judge_id: JudgeRunId,
    pub submission_id: SubmissionId,
    /*
    pub overall_judge_status: JudgeStatus,
    pub judge_progress_step: i32,
    pub total_score: i32,
    pub max_time_ms: i32,
    pub max_memory_mib: i32,
    */
    pub requested_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

/// TODO: use [`crate::domain::model::judge_state`]
pub struct JudgeResult {
    pub judge_id: JudgeRunId,
    pub testcase_id: TestcaseId,
    /*
    pub judge_status: JudgeStatus,
    pub score: i32,
    pub time_ms: i32,
    pub memory_mib: i32,
    */
}

/// TODO: use [`crate::domain::model::judge_state`]
pub struct CreateSubmission {
    pub problem_id: ProblemId,
    pub user_id: UserId,
    pub language_id: LanguageId,
    pub code_length_bytes: i32,
    /*
    pub overall_judge_status: JudgeStatus,
    pub judge_progress_step: i32,
    pub total_score: i32,
    pub max_time_ms: i32,
    pub max_memory_mib: i32,
    */
}

pub struct CreateSubmissionSource {
    pub submission_id: SubmissionId,
    pub source_bundle: Vec<u8>,
}

/// TODO: use [`crate::domain::model::judge_state`]
pub struct CreateJudgeRun {
    pub submission_id: SubmissionId,
    /*
    pub overall_judge_status: JudgeStatus,
    pub judge_progress_step: i32,
    pub total_score: i32,
    pub max_time_ms: i32,
    pub max_memory_mib: i32,
    */
}

/// TODO: use [`crate::domain::model::judge_state`]
pub struct UpdateJudgeRun {
    /*
    pub overall_judge_status: JudgeStatus,
    pub judge_progress_step: i32,
    pub total_score: i32,
    pub max_time_ms: i32,
    pub max_memory_mib: i32,
    */
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

/// TODO: use [`crate::domain::model::judge_state`]
pub struct CreateJudgeResult {
    pub judge_id: JudgeRunId,
    pub testcase_id: TestcaseId,
    /*
    pub judge_status: JudgeStatus,
    pub score: i32,
    pub time_ms: i32,
    pub memory_mib: i32,
    */
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
/// TODO: use [`crate::domain::model::judge_state`]
pub struct SubmissionGetQuery {
    pub user_id: Option<UserId>,
    pub limit: i64,
    pub offset: i64,
    /*
    pub judge_status: Option<JudgeStatus>,
    */
    pub language_id: Option<LanguageId>,
    pub user_name: Option<String>,
    pub user_query: Option<UserId>,
    pub order_by: SubmissionOrderBy,
    pub problem_id: Option<ProblemId>,
}
