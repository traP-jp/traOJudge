use anyhow::Result;
use async_trait::async_trait;

use crate::domain::model::problem::{
    CreateNormalProblem, NormalProblem, ProblemGetQuery, ProblemId, UpdateNormalProblem,
};

#[async_trait]
pub trait ProblemRepository: Send {
    async fn get_problem(&mut self, id: ProblemId) -> Result<Option<NormalProblem>>;
    async fn get_problems_by_query(&mut self, query: ProblemGetQuery)
    -> Result<Vec<NormalProblem>>;
    async fn get_problems_by_query_count(&mut self, query: ProblemGetQuery) -> Result<i64>;
    async fn create_problem(&mut self, problem: CreateNormalProblem) -> Result<ProblemId>;
    async fn update_problem(&mut self, id: ProblemId, problem: UpdateNormalProblem) -> Result<()>;
    async fn delete_problem(&mut self, id: ProblemId) -> Result<()>;
}
