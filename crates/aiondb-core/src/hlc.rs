use parking_lot::Mutex;
use std::cmp::max;
use std::time::{SystemTime, SystemTimeError, UNIX_EPOCH};

/// A Hybrid Logical Clock timestamp providing causal ordering across distributed nodes.
///
/// Combines a physical component (wall clock in milliseconds) with a logical counter
/// to guarantee strict monotonicity even when the wall clock does not advance between
/// two events on the same node, or when events arrive from nodes with clock skew.
///
/// Ordering is lexicographic: `physical` is compared first, then `logical`.
/// All timestamps in AionDB — `valid_from`, `valid_to`, `transaction_from`,
/// `transaction_to` — are `HLCTimestamp`. Direct use of `SystemTime` is forbidden
/// in library code.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialOrd,
    PartialEq,
    Ord,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    Default,
)]
pub struct HLCTimestamp {
    physical: u64, // system time in milliseconds since Unix epoch
    logical: u16,  // logical counter to order events coming during the same millisecond
}

impl HLCTimestamp {
    /// Creates a new `HLCTimestamp` from a physical time (milliseconds since Unix epoch)
    /// and a logical counter.
    pub fn new(physical: u64, logical: u16) -> HLCTimestamp {
        HLCTimestamp { physical, logical }
    }
}

/// Errors produced by [`HLC`] operations.
#[derive(thiserror::Error, Debug)]
pub enum HLCError {
    /// The logical counter reached `u16::MAX` and cannot be incremented.
    /// The node is processing more than 65 535 events within a single millisecond.
    #[error("Logical counter overflow at {0:?}")]
    LogicalOverflow(HLCTimestamp),

    /// The system clock returned an error (e.g. time went before the Unix epoch).
    #[error("system clock error")]
    SystemTimeError(#[from] SystemTimeError),
}

/// A Hybrid Logical Clock.
///
/// Maintains the last observed [`HLCTimestamp`] and produces strictly monotonic
/// timestamps on every call to [`HLC::now`]. Safe to share across threads via
/// `Arc<HLC>` — all mutation is internally synchronised with a [`parking_lot::Mutex`].
#[derive(Default)]
pub struct HLC {
    last: Mutex<HLCTimestamp>,
}

impl HLC {
    /// Creates a new [`HLC`] initialised at the zero timestamp.
    /// The first call to [`HLC::now`] will advance it to the current wall clock time.
    pub fn new() -> HLC {
        HLC {
            last: Mutex::new(HLCTimestamp::default()),
        }
    }

    /// Returns a new [`HLCTimestamp`] that is strictly greater than all previously
    /// returned timestamps and greater than or equal to the current wall clock time.
    ///
    /// # Errors
    /// - [`HLCError::LogicalOverflow`] if the logical counter would exceed `u16::MAX`.
    /// - [`HLCError::SystemTimeError`] if the system clock is unavailable.
    pub fn now(&self) -> Result<HLCTimestamp, HLCError> {
        let mut time = self.last.lock();
        let (physical, logical) = (time.physical, time.logical);
        let system_time = get_epoch_time_now()?;
        let result = if physical >= system_time {
            if logical < u16::MAX {
                Ok(HLCTimestamp::new(physical, logical + 1))
            } else {
                Err(HLCError::LogicalOverflow(*time))
            }
        } else {
            Ok(HLCTimestamp::new(system_time, 0))
        };
        if let Ok(t) = &result {
            *time = *t;
        }
        result
    }

    /// Advances the clock after receiving a message carrying timestamp `ts`.
    ///
    /// Ensures the local clock is strictly greater than both the previous local
    /// timestamp and the received `ts`, maintaining causal ordering across nodes.
    /// Call this on every inbound message before processing it.
    ///
    /// # Errors
    /// - [`HLCError::LogicalOverflow`] if the logical counter would exceed `u16::MAX`.
    pub fn observe(&self, ts: HLCTimestamp) -> Result<(), HLCError> {
        let mut time = self.last.lock();
        let result = if time.physical > ts.physical {
            if time.logical < u16::MAX {
                HLCTimestamp::new(time.physical, time.logical + 1)
            } else {
                return Err(HLCError::LogicalOverflow(*time));
            }
        } else if time.physical == ts.physical {
            let new_logical = max(time.logical as u32, ts.logical as u32) + 1;
            if new_logical > u16::MAX as u32 {
                return Err(HLCError::LogicalOverflow(HLCTimestamp::new(
                    ts.physical,
                    new_logical as u16,
                )));
            } else {
                HLCTimestamp::new(time.physical, new_logical as u16)
            }
        } else {
            let new_logical = if ts.logical < u16::MAX {
                ts.logical + 1
            } else {
                return Err(HLCError::LogicalOverflow(ts));
            };
            HLCTimestamp::new(ts.physical, new_logical)
        };
        *time = result;
        Ok(())
    }
}

fn get_epoch_time_now() -> Result<u64, SystemTimeError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|t| t.as_millis() as u64)
}
