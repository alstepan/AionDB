use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::{identity::RowId, interval::ValidInterval};

/// Opaque serialised bytes representing user data. Interpretation is governed by [`SchemaId`].
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct Payload(pub Vec<u8>);

/// SchemaId governs interpretation of the payload stored in the record. SchemaId is local to the database engine.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, PartialOrd, Eq, Ord, Debug)]
pub struct SchemaId(pub u32);

/// The temporal data record
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct Record {
    /// Schema version governing payload interpretation. Local to this node's registry.
    pub schema_id: SchemaId,
    /// Globally unique row identifier, assigned by the storage engine at insertion time.
    pub row_id: RowId,
    /// The half-open interval [valid_from, valid_to) during which this record is valid in the real world.
    pub valid_interval: ValidInterval,
    /// Opaque serialised bytes. Interpretation is governed by SchemaId
    pub payload: Payload,
}

impl Display for SchemaId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "schema:{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_id_renders_correctly() {
        assert_eq!(SchemaId(123).to_string(), "schema:123".to_string());
    }
}
