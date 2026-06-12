use std::collections::HashMap;

use crate::domain::model::file::FileId;

#[derive(Debug, Clone)]
pub struct GenericDag {
    pub nodes: HashMap<NodeId, Node>,
    pub edges: Vec<Edge>,
    /// Set of envvars which will be broadcasted to all execution nodes.
    pub fixed_env_vars: HashMap<String, String>,
    pub dynamic_env_vars: Vec<String>,
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
    DynamicFile(DynamicFileNode),
    Execution(ExecutionNode),
}

/// Testcases, checker, etc.
#[derive(Debug, Clone)]
pub struct FixedFileNode {
    pub file_id: FileId,
}

/// Submission-specific file.
#[derive(Debug, Clone)]
pub struct DynamicFileNode {
    pub name: String,
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
