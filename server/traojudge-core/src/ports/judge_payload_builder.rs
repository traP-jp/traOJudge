use traojudge_workflow::payload::{Payload, PayloadError, PayloadRevision};

use crate::domain::model::judge_dag::ExecutionNode;

pub trait JudgePayloadBuilder {
    type Input: PayloadRevision;

    fn make_payload(
        &mut self,
        index: usize,
        execution: &ExecutionNode,
    ) -> Result<Payload<Self::Input>, PayloadError>;
}
