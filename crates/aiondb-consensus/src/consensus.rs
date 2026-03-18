use async_trait::async_trait;

use crate::types::{LogEntry, LogIndex};

#[derive(thiserror::Error, Debug)]
pub enum ConsensusError {
    #[error("not a leader")]
    NotLeader,

    #[error("consensus unavailable")]
    Unavailable,
}

#[async_trait]
pub trait Consensus: Send + Sync {
    async fn propose(&self, entry: LogEntry) -> Result<LogIndex, ConsensusError>;
}
