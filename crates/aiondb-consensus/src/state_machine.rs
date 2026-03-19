use async_trait::async_trait;

use crate::log::LogEntry;

#[derive(thiserror::Error, Debug)]
pub enum StateMachineError {
    #[error("apply failed: {0}")]
    ApplyFailed(String),
}

#[async_trait]
pub trait StateMachine: Send + Sync {
    async fn apply(&self, entry: LogEntry) -> Result<(), StateMachineError>;
}
