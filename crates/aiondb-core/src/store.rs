use std::sync::Arc;

use parking_lot::Mutex;

use crate::{
    hlc::HLCTimestamp,
    identity::RowId,
    interval::ValidInterval,
    record::{Record, SchemaId},
};

/// Error produced by [`Store`]
#[derive(thiserror::Error, Debug)]
pub enum StoreError {
    // reserved for later phases
    #[allow(dead_code)]
    #[error("Cannot insert record {0}, {1} due to {2}")]
    InsertFailure(SchemaId, RowId, String),

    #[allow(dead_code)] // reserved for later phases
    #[error("Cannot query the store at time {0}")]
    QueryAsOfTimeFailure(HLCTimestamp),

    #[allow(dead_code)] // reserved for later phases
    #[error("Cannot query the store at range {0}")]
    RangeQueryFailure(ValidInterval),
}

pub type StoreResultSet = Result<Vec<Arc<Record>>, StoreError>;

/// Contains all operations that can be performed over the store
pub trait Store {
    /// Inserts a record `r` into the store.
    ///
    /// # Errors
    /// - [`StoreError::InsertFailure`] if the insert operation is not possible.
    fn insert(&self, r: Record) -> Result<(), StoreError>;

    /// Queries records that are valid at given time `ts`
    ///
    /// # Errors
    /// - [`StoreError::QueryAsOfTimeFailure`] if the query is not possible.
    fn query_as_of(&self, ts: HLCTimestamp) -> StoreResultSet;

    /// Queries records that are valid at given interval `valid`
    ///
    /// # Errors
    /// - [`StoreError::RangeQueryFailure`] if a range query is not possible.
    fn query_range(&self, valid: &ValidInterval) -> StoreResultSet;
}

/// Naive in-memory store implementation. Supports only scan queries
/// `Default` yields empty [`MemStore`].
#[derive(Default)]
pub struct MemStore {
    records: Mutex<Vec<Arc<Record>>>,
}

impl MemStore {
    /// Creates empty [`MemStore`] with no records.
    pub fn new() -> Self {
        MemStore {
            records: Mutex::new(vec![]),
        }
    }
}

impl Store for MemStore {
    fn insert(&self, r: Record) -> Result<(), StoreError> {
        let mut records = self.records.lock();
        let record = Arc::new(r);
        records.push(record);
        Ok(())
    }

    fn query_as_of(&self, ts: HLCTimestamp) -> StoreResultSet {
        let records = self.records.lock();
        let result = records
            .iter()
            .filter(|r| r.valid_interval.contains(ts))
            .cloned()
            .collect();
        Ok(result)
    }

    fn query_range(&self, valid: &ValidInterval) -> StoreResultSet {
        let records = self.records.lock();
        let result = records
            .iter()
            .filter(|r| r.valid_interval.overlaps(valid))
            .cloned()
            .collect();
        Ok(result)
    }
}
