use std::collections::HashMap;

use crate::domain::model::file::StoreFileId;

pub const TRAOJUDGE_SP_TIME_LIMIT: &str = "TRAOJUDGE_SP_TIME_LIMIT";
pub const TRAOJUDGE_SP_MEMORY_LIMIT: &str = "TRAOJUDGE_SP_MEMORY_LIMIT";
pub const TRAOJUDGE_SP_OUTPUT_LIMIT: &str = "TRAOJUDGE_SP_OUTPUT_LIMIT";
pub const TRAOJUDGE_SP_LANGUAGE: &str = "TRAOJUDGE_SP_LANGUAGE";

#[derive(Debug, Clone)]
pub struct TwoPlayerDag {
    pub nodes: HashMap<NodeId, Node>,
    pub edges: Vec<Edge>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(i32);

impl From<i32> for NodeId {
    fn from(id: i32) -> Self {
        Self(id)
    }
}

impl Into<i32> for NodeId {
    fn into(self) -> i32 {
        self.0
    }
}

#[derive(Debug, Clone)]
pub enum Node {
    FixedFile(FixedFileNode),
    SubmissionSourceFile(SubmissionSourceFileNode),
    Execution(ExecutionNode),
}

/// Testcases, checker, etc.
#[derive(Debug, Clone)]
pub struct FixedFileNode {
    pub file_id: StoreFileId,
}

#[derive(Debug, Clone)]
pub enum SubmissionSourceFileNode {
    Alice,
    Bob,
}

#[derive(Debug, Clone)]
pub struct ExecutionNode {
    pub command: String,
}

#[derive(Debug, Clone)]
pub struct Edge {
    pub source: NodeId,
    pub target: NodeId,
    pub env_key: String,
}
