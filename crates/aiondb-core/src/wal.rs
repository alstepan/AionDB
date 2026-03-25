use serde::{Deserialize, Serialize};

use crate::{
    identity::NodeId,
    replication::{LogIndex, Term},
};

/// CRC 32 to verify the data integrity
#[derive(PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct CRC(pub u32);

/// The replication metadata header written to the WAL before each record.
/// On single-node `term`=0, `log_index` is monotonically increasing
#[derive(Serialize, Deserialize, Debug)]
pub struct WalEntry {
    /// The replication term in which this entry was proposed.
    pub term: Term,
    /// The position of this entry in the replication log.
    pub log_index: LogIndex,
    /// The node that originated this entry.
    pub node_id: NodeId,
}

/// The on-disk envelope for a single WAL record.
/// Used for recovery: length is scanned to find entry boundaries, crc detects corruption
#[derive(Serialize, Deserialize, Debug)]
pub struct WalFrame {
    /// Data only length
    pub length: u32,
    /// CRC for protection from the data corruption
    pub crc: CRC,
    /// Serialized data
    pub data: Vec<u8>,
}
