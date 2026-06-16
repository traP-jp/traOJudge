use anyhow::Result;
use async_trait::async_trait;

use crate::domain::model::{
    problem::ProblemId,
    testcase::{CreateTestcase, TestcaseId, TestcaseSummary},
};

#[async_trait]
pub trait TestcaseRepository: Send {
    async fn get_testcases(&mut self, problem_id: ProblemId) -> Result<Vec<TestcaseSummary>>;
    async fn get_testcase(&mut self, id: TestcaseId) -> Result<Option<TestcaseSummary>>;
    async fn create_testcases(&mut self, testcases: Vec<CreateTestcase>) -> Result<()>;
    async fn delete_testcases(&mut self, problem_id: ProblemId) -> Result<()>;
}
