use aiondb_core::identity::NodeId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LogIndex(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Term(u64);

#[derive(Hash, Clone)]
pub struct Payload(pub Vec<u8>);

#[derive(Hash, Clone)]
pub struct LogEntry{
    pub term: Term,
    pub log_index: LogIndex,
    pub node_id: NodeId,
    pub payload: Payload
}
