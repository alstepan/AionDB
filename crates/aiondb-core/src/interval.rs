use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::hlc::HLCTimestamp;

/// A half-open time interval `[valid_from, valid_to)` over [`HLCTimestamp`].
///
/// Used to represent both storage records and query range expressions.
///
/// - `valid_to = HLCTimestamp::MAX` means the record is currently valid with no known end.
/// - `valid_from = HLCTimestamp::MIN` is meaningful only in query range expressions
///   (e.g. "from the beginning of history"). Storage records must carry the actual
///   [`HLCTimestamp`] at which the record became valid.
#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize, Clone)]
pub struct ValidInterval {
    pub valid_from: HLCTimestamp, // HLCTimestamp::MIN means "open" / from the start of the time
    pub valid_to: HLCTimestamp,   // HLCTimestamp::MAX means "open" / current
}

impl Display for ValidInterval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}, {})", self.valid_from, self.valid_to)
    }
}

/// Errors produced by [`ValidInterval`] operations.
#[derive(thiserror::Error, Debug)]
pub enum IntervalError {
    #[error("valid_from {0} is after valid_to {1}")]
    InvalidInterval(HLCTimestamp, HLCTimestamp),
}

impl ValidInterval {
    /// Creates a new ValidInterval.
    ///
    /// `from` should be less or equal than `to`
    ///
    /// # Errors
    ///
    /// Returns Err(IntervalError::InvalidInterval) if from > to
    pub fn new(from: HLCTimestamp, to: HLCTimestamp) -> Result<Self, IntervalError> {
        if to < from {
            Err(IntervalError::InvalidInterval(from, to))
        } else {
            Ok(Self {
                valid_from: from,
                valid_to: to,
            })
        }
    }

    /// Creates a new open ValidInterval
    ///
    /// `valid_to` is set to [`HLCTimestamp::MAX`]. This means that interval is currently valid , no known end
    pub fn open(from: HLCTimestamp) -> Self {
        Self {
            valid_from: from,
            valid_to: HLCTimestamp::MAX,
        }
    }

    /// Verifies if the timestamp is withing the current [`ValidInterval`]
    pub fn contains(&self, ts: HLCTimestamp) -> bool {
        (self.valid_from <= ts) && (self.valid_to > ts)
    }

    /// Verifies if other ValidInterval overlaps with the current
    pub fn overlaps(&self, other: &ValidInterval) -> bool {
        self.valid_from < other.valid_to && other.valid_from < self.valid_to
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_creates_open_interval() {
        let from = HLCTimestamp::new(123, 456);
        let i = ValidInterval::open(from);

        assert_eq!(i.valid_from, from);
        assert_eq!(i.valid_to, HLCTimestamp::MAX);
    }

    #[test]
    fn test_creates_valid_interval() {
        let from = HLCTimestamp::new(123, 456);
        let to = HLCTimestamp::new(456, 789);
        let i = ValidInterval::new(from, to).unwrap();

        assert_eq!(i.valid_from, from);
        assert_eq!(i.valid_to, to);
    }

    #[test]
    fn test_creates_one_point_interval() {
        let from = HLCTimestamp::new(123, 456);
        let i = ValidInterval::new(from, from).unwrap();

        assert_eq!(i.valid_from, from);
        assert_eq!(i.valid_to, from);
    }

    #[test]
    fn test_returns_error_when_from_greater_than_to() {
        let to = HLCTimestamp::new(123, 456);
        let from = HLCTimestamp::new(456, 789);
        let i = ValidInterval::new(from, to);

        assert!(matches!(
            i.unwrap_err(),
            IntervalError::InvalidInterval(_from, _to)
        ));
    }

    #[test]
    fn test_renders_to_string() {
        let from = HLCTimestamp::new(123, 456);
        let to = HLCTimestamp::new(456, 789);
        let i = ValidInterval::new(from, to).map(|e| e.to_string());

        assert_eq!(
            i.unwrap(),
            "[1970-01-01T00:00:00.123Z.456, 1970-01-01T00:00:00.456Z.789)".to_string()
        );
    }

    #[test]
    fn test_contains() {
        let from = HLCTimestamp::new(123, 456);
        let to = HLCTimestamp::new(456, 789);
        let i = ValidInterval::new(from, to).unwrap();
        let ts = HLCTimestamp::new(234, 189);
        let ts2 = HLCTimestamp::new(122, 123);
        let ts3 = HLCTimestamp::new(456, 999);

        assert!(i.contains(ts));
        assert!(!i.contains(ts2));
        assert!(!i.contains(ts3));
        assert!(i.contains(from));
        assert!(!i.contains(to));
    }

    #[test]
    fn test_overlaps() {
        let i =
            ValidInterval::new(HLCTimestamp::new(123, 456), HLCTimestamp::new(456, 789)).unwrap();

        // interval overlaps on right boundary
        let o1 =
            ValidInterval::new(HLCTimestamp::new(455, 456), HLCTimestamp::new(458, 789)).unwrap();

        // interval containing i
        let o2 =
            ValidInterval::new(HLCTimestamp::new(100, 456), HLCTimestamp::new(458, 789)).unwrap();

        // interval overlaps on left boundary by just logical tick
        let o3 =
            ValidInterval::new(HLCTimestamp::new(100, 456), HLCTimestamp::new(123, 457)).unwrap();

        // interval containing i
        let o4 =
            ValidInterval::new(HLCTimestamp::new(100, 456), HLCTimestamp::new(500, 457)).unwrap();

        // interval inside i
        let o5 =
            ValidInterval::new(HLCTimestamp::new(127, 456), HLCTimestamp::new(400, 457)).unwrap();

        // interval before i with with the same point in time
        let o6 =
            ValidInterval::new(HLCTimestamp::new(100, 456), HLCTimestamp::new(123, 456)).unwrap();

        // interval after i with the same point in time
        let o7 =
            ValidInterval::new(HLCTimestamp::new(456, 789), HLCTimestamp::new(790, 456)).unwrap();

        assert!(i.overlaps(&o1));
        assert!(i.overlaps(&o2));
        assert!(i.overlaps(&o3));
        assert!(i.overlaps(&o4));
        assert!(i.overlaps(&o5));

        assert!(!i.overlaps(&o6));
        assert!(!i.overlaps(&o7));
    }

    #[test]
    fn test_overlaps_open_interval() {
        let i = ValidInterval::open(HLCTimestamp::new(123, 456));

        let o1 =
            ValidInterval::new(HLCTimestamp::new(100, 0), HLCTimestamp::new(123, 456)).unwrap();
        let o2 =
            ValidInterval::new(HLCTimestamp::new(123, 456), HLCTimestamp::new(789, 123)).unwrap();
        let o3 = ValidInterval::open(HLCTimestamp::new(456, 456));

        assert!(!i.overlaps(&o1));
        assert!(i.overlaps(&o2));
        assert!(i.overlaps(&o3));
        assert!(i.overlaps(&i));
    }
}
