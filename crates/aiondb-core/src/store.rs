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

#[cfg(test)]
mod tests {
    use crate::{identity::NodeId, record::Payload};

    use super::*;

    #[test]
    fn test_insert_adds_records() {
        let store = MemStore::new();
        let r1 = Record {
            row_id: RowId::new(HLCTimestamp::new(1, 1), NodeId(1)),
            schema_id: SchemaId(1),
            valid_interval: ValidInterval::new(HLCTimestamp::new(1, 1), HLCTimestamp::new(2, 3))
                .unwrap(),
            payload: Payload(vec![1, 2, 3, 4]),
        };
        let r1_1 = r1.clone();

        store.insert(r1).unwrap();

        let result = store.query_as_of(HLCTimestamp::new(2, 1));

        let res = result
            .unwrap()
            .first()
            .expect("One record is expected")
            .clone();

        assert_eq!(*res, r1_1);
    }

    #[test]
    fn test_query_as_of_returns_empty_result_when_timestamp_is_outside_the_interval() {
        let store = MemStore::new();
        let r1 = Record {
            row_id: RowId::new(HLCTimestamp::new(1, 1), NodeId(1)),
            schema_id: SchemaId(1),
            valid_interval: ValidInterval::new(HLCTimestamp::new(3, 1), HLCTimestamp::new(5, 3))
                .unwrap(),
            payload: Payload(vec![1, 2, 3, 4]),
        };

        store.insert(r1).unwrap();

        let result = store.query_as_of(HLCTimestamp::new(2, 1));
        let res = result.unwrap();

        assert!(res.is_empty());
    }

    #[test]
    fn test_empty_store_returns_empty_results() {
        let store = MemStore::new();

        let result = store.query_as_of(HLCTimestamp::new(2, 1));
        let res = result.unwrap();

        assert!(res.is_empty());

        let result = store.query_range(&ValidInterval::open(HLCTimestamp::MIN));
        let res = result.unwrap();

        assert!(res.is_empty());
    }

    #[test]
    fn test_query_range_returns_matching_records() {
        let store = MemStore::new();
        let r1 = Record {
            row_id: RowId::new(HLCTimestamp::new(1, 1), NodeId(1)),
            schema_id: SchemaId(1),
            valid_interval: ValidInterval::new(HLCTimestamp::new(3, 1), HLCTimestamp::new(3, 2))
                .unwrap(),
            payload: Payload(vec![1, 2, 3, 4]),
        };
        let r2 = Record {
            row_id: RowId::new(HLCTimestamp::new(1, 2), NodeId(1)),
            schema_id: SchemaId(1),
            valid_interval: ValidInterval::new(HLCTimestamp::new(4, 1), HLCTimestamp::new(5, 3))
                .unwrap(),
            payload: Payload(vec![1, 2, 3, 4]),
        };
        let r2_2 = r2.clone();

        store.insert(r1).expect("cannot insert record");
        store.insert(r2).expect("cannot insert record");

        let result = store
            .query_range(&ValidInterval::open(HLCTimestamp::new(4, 0)))
            .unwrap();

        assert_eq!(result.len(), 1);

        let res = result.first().expect("Expected one record").clone();

        assert_eq!(*res, r2_2);
    }
}
