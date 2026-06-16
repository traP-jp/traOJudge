use anyhow::Result;
use async_trait::async_trait;

use crate::domain::model::problem::{
    CreateProblem, Problem, ProblemGetQuery, ProblemId, UpdateProblem,
};

#[async_trait]
pub trait ProblemRepository: Send {
    async fn get_problem(&mut self, id: ProblemId) -> Result<Option<Problem>>;
    async fn get_problems_by_query(&mut self, query: ProblemGetQuery) -> Result<Vec<Problem>>;
    async fn get_problems_count_by_query(&mut self, query: ProblemGetQuery) -> Result<i64>;
    async fn create_problem(&mut self, problem: CreateProblem) -> Result<ProblemId>;
    async fn update_problem(&mut self, id: ProblemId, problem: UpdateProblem) -> Result<()>;
    async fn delete_problem(&mut self, id: ProblemId) -> Result<()>;
}
