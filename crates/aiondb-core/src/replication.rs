use serde::{Deserialize, Serialize};

/// Monotonically increasing position of an entry in the Raft log.
///
/// Assigned by the leader at proposal time. Two entries with the same
/// `LogIndex` on different nodes are guaranteed to have identical content
/// (Raft log matching property).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct LogIndex(pub u64);

/// Raft election term. Increments each time a new leader is elected.
///
/// A higher term always supersedes a lower one. Entries from an earlier
/// term that were not committed before a leadership change will be
/// overwritten by the new leader.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Term(pub u64);
