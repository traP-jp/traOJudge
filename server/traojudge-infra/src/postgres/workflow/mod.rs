//! PostgreSQLをstorage backendとして使うワークフロー実装。
//!
//! テーブルは `crate::postgres::MIGRATOR` が `src/postgres/migrations` から作る。

mod edge_repository;
mod job_repository;
mod pipeline_repository;
mod unit_of_work;

pub use edge_repository::PostgresEdgeRepository;
pub use job_repository::PostgresJobRepository;
pub use pipeline_repository::PostgresPipelineRepository;
pub use unit_of_work::{PostgresUnitOfWork, PostgresUnitOfWorkProvider};
