use aiondb_core::{
    identity::NodeId,
    replication::{LogIndex, Term},
};
use serde::{Deserialize, Serialize};

/// Opaque serialised bytes representing a storage engine operation.
///
/// The consensus layer replicates this payload without interpreting its
/// contents. The storage engine is responsible for serialising before
/// `propose()` and deserialising inside `StateMachine::apply()`.
#[derive(Hash, Clone, Debug, Serialize, Deserialize)]
pub struct Payload(pub Vec<u8>);

/// A single entry in the Raft log.
///
/// Carries the Raft protocol fields required for distributed correctness
/// alongside an opaque [`Payload`]. On a single-node deployment `term` is
/// `0` and `log_index` starts at `1`; both become load-bearing once Raft
/// is implemented in Phase 10.
#[derive(Hash, Clone, Debug, Serialize, Deserialize)]
pub struct LogEntry {
    /// The Raft term in which this entry was proposed.
    pub term: Term,
    /// The position of this entry in the Raft log.
    pub log_index: LogIndex,
    /// The node that originated this entry.
    pub node_id: NodeId,
    /// Serialised storage engine operation — opaque to the consensus layer.
    pub payload: Payload,
}
