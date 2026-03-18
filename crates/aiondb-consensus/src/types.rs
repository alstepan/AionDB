use aiondb_core::identity::NodeId;

/// Monotonically increasing position of an entry in the Raft log.
///
/// Assigned by the leader at proposal time. Two entries with the same
/// `LogIndex` on different nodes are guaranteed to have identical content
/// (Raft log matching property).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LogIndex(pub u64);

/// Raft election term. Increments each time a new leader is elected.
///
/// A higher term always supersedes a lower one. Entries from an earlier
/// term that were not committed before a leadership change will be
/// overwritten by the new leader.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Term(pub u64);

/// Opaque serialised bytes representing a storage engine operation.
///
/// The consensus layer replicates this payload without interpreting its
/// contents. The storage engine is responsible for serialising before
/// `propose()` and deserialising inside `StateMachine::apply()`.
#[derive(Hash, Clone, Debug)]
pub struct Payload(pub Vec<u8>);

/// A single entry in the Raft log.
///
/// Carries the Raft protocol fields required for distributed correctness
/// alongside an opaque [`Payload`]. On a single-node deployment `term` is
/// `0` and `log_index` starts at `1`; both become load-bearing once Raft
/// is implemented in Phase 10.
#[derive(Hash, Clone, Debug)]
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
