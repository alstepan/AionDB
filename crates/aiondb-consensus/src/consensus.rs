use aiondb_core::replication::LogIndex;
use async_trait::async_trait;

use crate::log::LogEntry;

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
