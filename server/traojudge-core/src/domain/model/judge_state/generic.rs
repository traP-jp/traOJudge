use std::collections::HashMap;

use crate::domain::model::{dag::generic_dag::NodeId, submission::JudgeRunId};

pub struct JudgeRunState {
    pub judge_run_id: JudgeRunId,
    pub dynamic_env_vars: HashMap<String, String>,
    pub nodes: JudgeRunNode,
}

pub struct JudgeRunNode {
    node_id: NodeId,
    status: JudgeRunNodeStatus,
}

/// [`JudgingNodeStatus`] does not have Failed state because
/// failed judge runs will immediately be aborted.
pub enum JudgeRunNodeStatus {
    Waiting,
    Running,
    Completed,
}
