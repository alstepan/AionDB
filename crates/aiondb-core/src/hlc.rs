use parking_lot::Mutex;
use std::cmp::max;
use std::fmt::Display;
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

impl Display for HLCTimestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(phys) = i64::try_from(self.physical)
            .ok()
            .and_then(chrono::DateTime::from_timestamp_millis)
        {
            write!(
                f,
                "{}.{}",
                phys.format("%Y-%m-%dT%H:%M:%S%.3fZ"),
                self.logical
            )
        } else {
            write!(f, "Invalid time: {}.{}", self.physical, self.logical)
        }
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

/// A type defining system clock
pub type ClockFn = Mutex<Box<dyn FnMut() -> Result<u64, HLCError>>>;
/// A Hybrid Logical Clock.
///
/// Maintains the last observed [`HLCTimestamp`] and produces strictly monotonic
/// timestamps on every call to [`HLC::now`]. Safe to share across threads via
/// `Arc<HLC>` — all mutation is internally synchronised with a [`parking_lot::Mutex`].
pub struct HLC {
    last: Mutex<HLCTimestamp>,
    // Lock ordering: `clock` and `last` must never be held simultaneously.
    // Always release one before acquiring the other.    
    clock: ClockFn,
}

impl Default for HLC {
    fn default() -> Self {
        Self {
            last: Default::default(),
            clock: Mutex::new(Box::new(get_epoch_time_now)),
        }
    }
}

impl HLC {
    /// Creates a new [`HLC`] initialised at the zero timestamp.
    /// The first call to [`HLC::now`] will advance it to the current wall clock time.
    pub fn new() -> HLC {
        HLC::default()
    }

    /// Returns a new [`HLCTimestamp`] that is strictly greater than all previously
    /// returned timestamps and greater than or equal to the current wall clock time.
    ///
    /// # Errors
    /// - [`HLCError::LogicalOverflow`] if the logical counter would exceed `u16::MAX`.
    /// - [`HLCError::SystemTimeError`] if the system clock is unavailable.
    pub fn now(&self) -> Result<HLCTimestamp, HLCError> {
        let mut sys_clock = self.clock.lock();
        let system_time = (sys_clock)()?;
        // Release clock lock before acquiring `last` — never hold both simultaneousl        
        drop(sys_clock);
        let mut time = self.last.lock();
        let (physical, logical) = (time.physical, time.logical);
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

fn get_epoch_time_now() -> Result<u64, HLCError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|t| t.as_millis() as u64)
        .map_err(|e| HLCError::SystemTimeError(e))
}

#[cfg(test)]
mod tests {
    use super::*;

    impl HLC {
        fn last_time(&self) -> HLCTimestamp {
            let time = self.last.lock();
            *time
        }

        fn with_clock(clock: Box<dyn FnMut() -> Result<u64, HLCError>>) -> HLC {
            HLC {
                last: Mutex::new(HLCTimestamp::default()),
                clock: Mutex::new(clock),
            }
        }

        fn set_time(&self, new_time: HLCTimestamp) {
            let mut time = self.last.lock();
            *time = new_time;
        }
    }

    #[test]
    fn test_hlc_timestamp_displays_correctly() {
        let timestamp = HLCTimestamp::new(1773167264000, 3);
        assert_eq!(format!("{}", timestamp), "2026-03-10T18:27:44.000Z.3")
    }

    #[test]
    fn test_hlc_timestamp_diplays_error() {
        let timestamp = HLCTimestamp::new(977316726400000999, 1);
        assert_eq!(
            format!("{}", timestamp),
            "Invalid time: 977316726400000999.1"
        )
    }

    #[test]
    fn test_hlc_now_increasing_logical_monotonically() {
        let clock = Box::new(|| Ok(1u64));
        let hlc = HLC::with_clock(clock);

        let res1 = hlc.now().unwrap();
        let res2 = hlc.now().unwrap();

        assert!(res2 > res1);

        hlc.set_time(HLCTimestamp::new(1, 1));
        let res3 = hlc.now().unwrap();

        assert!(res3 > HLCTimestamp::new(1, 1))
    }

    #[test]
    fn test_hlc_now_increasing_monotonically() {
        let mut clock_state: u64 = 1_000_000; 
        let clock = Box::new(move || {clock_state += 1000; Ok(clock_state)});
        let hlc = HLC::with_clock(clock);

        let res1 = hlc.now().unwrap();
        let res2 = hlc.now().unwrap();

        assert!(res2 > res1);

        let mut clock_state: u64 = 1_000_000; 
        let clock = Box::new(move || {clock_state -= 1000; Ok(clock_state)});
        let hlc = HLC::with_clock(clock);

        let res1 = hlc.now().unwrap();
        let res2 = hlc.now().unwrap();

         assert!(res2 > res1);
    }    

    #[test]
    fn test_hlc_now_handles_overflow() {
        let clock =Box::new( || Ok(1u64));
        let hlc = HLC::with_clock(clock);

        hlc.set_time(HLCTimestamp::new(1, u16::MAX));

        let res = hlc.now();

        assert!(res.is_err());
    }
}
