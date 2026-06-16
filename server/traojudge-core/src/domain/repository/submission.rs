use anyhow::Result;
use async_trait::async_trait;

use crate::domain::model::submission::{
    CreateJudgeResult, CreateSubmission, JudgeResult, Submission, SubmissionGetQuery, SubmissionId,
    UpdateSubmission,
};

#[async_trait]
pub trait SubmissionRepository: Send {
    async fn get_submission(&mut self, id: SubmissionId) -> Result<Option<Submission>>;
    async fn get_submission_results(&mut self, id: SubmissionId) -> Result<Vec<JudgeResult>>;
    async fn get_submissions_by_query(
        &mut self,
        query: SubmissionGetQuery,
    ) -> Result<Vec<Submission>>;
    async fn get_submissions_count_by_query(&mut self, query: SubmissionGetQuery) -> Result<i64>;
    async fn create_submission(&mut self, submission: CreateSubmission) -> Result<SubmissionId>;
    async fn update_submission(
        &mut self,
        submission_id: SubmissionId,
        submission: UpdateSubmission,
    ) -> Result<()>;
    async fn create_judge_results(&mut self, results: Vec<CreateJudgeResult>) -> Result<()>;
    async fn delete_judge_results_by_submission_id(
        &mut self,
        submission_id: SubmissionId,
    ) -> Result<()>;
}
