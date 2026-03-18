use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::hlc::HLCTimestamp;

/// A unique identifier for a node in the AionDB cluster.
///
/// Wraps a `u64` for type safety — prevents accidental assignment to unrelated integer fields.
/// `NodeId(0)` is a valid identifier; there is no sentinel value.
#[derive(PartialEq, Eq, Hash, Serialize, Deserialize, Clone, Copy, Debug, PartialOrd, Ord)]
pub struct NodeId(pub u64);

/// A globally unique identifier for a record, composed of the originating node and its
/// [`HLCTimestamp`] at the time of insertion.
///
/// Ordering is timestamp-first, then node-id — enabling efficient time-range scans over
/// sorted indexes. Constructed exclusively by the storage engine via [`RowId::new`].
#[derive(PartialEq, Eq, Hash, Serialize, Deserialize, Clone, Copy, Debug, PartialOrd, Ord)]
pub struct RowId {
    pub timestamp: HLCTimestamp,
    pub node_id: NodeId,
}

impl Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "node:{}", self.0)
    }
}

impl Display for RowId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "rowId:{}:{}", self.timestamp, self.node_id)
    }
}

impl RowId {
    /// Creates a new [`RowId`] from a [`HLCTimestamp`] and the [`NodeId`] of the writing node.
    pub fn new(timestamp: HLCTimestamp, node_id: NodeId) -> RowId {
        Self { timestamp, node_id }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_nodeid_displays_correctly() {
        assert_eq!(NodeId(1234).to_string(), "node:1234".to_string())
    }

    #[test]
    fn test_rowid_displays_correctly() {
        assert_eq!(
            RowId::new(HLCTimestamp::new(12345, 3), NodeId(54321)).to_string(),
            "rowId:1970-01-01T00:00:12.345Z.3:node:54321".to_string()
        )
    }

    #[test]
    fn test_timestamp_first_ordering() {
        let ts_earlier = HLCTimestamp::new(0, 1);
        let ts_later = HLCTimestamp::new(0, 2);
        assert!(RowId::new(ts_earlier, NodeId(999)) < RowId::new(ts_later, NodeId(0)));
        assert!(RowId::new(ts_earlier, NodeId(999)) < RowId::new(ts_later, NodeId(999)));
    }
}
