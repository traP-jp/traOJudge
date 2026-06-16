use anyhow::Result;
use async_trait::async_trait;

use crate::domain::model::submission::{
    CreateJudgeResult, CreateJudgeRun, CreateSubmission, CreateSubmissionSource, JudgeResult,
    JudgeRunId, Submission, SubmissionGetQuery, SubmissionId, SubmissionJudgeRun, SubmissionSource,
    SubmissionSummary, UpdateJudgeRun,
};

#[async_trait]
pub trait SubmissionRepository: Send {
    async fn get_submission(&mut self, id: SubmissionId) -> Result<Option<Submission>>;
    async fn get_source(&mut self, id: SubmissionId) -> Result<Option<SubmissionSource>>;
    async fn get_submission_current_judge_results(
        &mut self,
        id: SubmissionId,
    ) -> Result<Vec<JudgeResult>>;
    async fn get_submissions_by_query(
        &mut self,
        query: SubmissionGetQuery,
    ) -> Result<Vec<SubmissionSummary>>;
    async fn get_submissions_count_by_query(&mut self, query: SubmissionGetQuery) -> Result<i64>;
    async fn create_submission(&mut self, submission: CreateSubmission) -> Result<SubmissionId>;
    async fn create_source(&mut self, source: CreateSubmissionSource) -> Result<()>;
    async fn create_judge_run(&mut self, judge_run: CreateJudgeRun) -> Result<JudgeRunId>;
    async fn get_judge_run(&mut self, judge_id: JudgeRunId) -> Result<Option<SubmissionJudgeRun>>;
    async fn get_judge_runs_by_submission_id(
        &mut self,
        submission_id: SubmissionId,
    ) -> Result<Vec<SubmissionJudgeRun>>;

    /// Updates a judge run.
    ///
    /// If the updated judge run is the submission's current run, implementations
    /// must also synchronize the denormalized judge summary columns on
    /// `submissions` in the same transaction.
    async fn update_judge_run(
        &mut self,
        judge_id: JudgeRunId,
        judge_run: UpdateJudgeRun,
    ) -> Result<()>;

    async fn update_current_judge_run(
        &mut self,
        submission_id: SubmissionId,
        judge_id: JudgeRunId,
    ) -> Result<()>;
    async fn create_judge_results(&mut self, results: Vec<CreateJudgeResult>) -> Result<()>;
    async fn get_judge_results_by_judge_id(
        &mut self,
        judge_id: JudgeRunId,
    ) -> Result<Vec<JudgeResult>>;
}
