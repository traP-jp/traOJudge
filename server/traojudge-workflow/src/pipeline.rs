use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PipelineId(Uuid);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineState {
    Running,
    Completed,
    Cancelled,
}

impl PipelineId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl Default for PipelineId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Uuid> for PipelineId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl From<PipelineId> for Uuid {
    fn from(value: PipelineId) -> Self {
        value.0
    }
}

impl fmt::Display for PipelineId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}
