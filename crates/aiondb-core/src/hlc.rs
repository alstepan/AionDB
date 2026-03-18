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
    Hash,
)]
pub struct HLCTimestamp {
    pub physical: u64, // system time in milliseconds since Unix epoch
    logical: u16,      // logical counter to order events coming during the same millisecond
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
pub(crate) type ClockFn = Box<dyn FnMut() -> Result<u64, HLCError> + Send>;
/// A Hybrid Logical Clock.
///
/// Maintains the last observed [`HLCTimestamp`] and produces strictly monotonic
/// timestamps on every call to [`HLC::now`]. Safe to share across threads via
/// `Arc<HLC>` — all mutation is internally synchronised with a [`parking_lot::Mutex`].
pub struct HLC {
    last: Mutex<HLCTimestamp>,
    // Lock ordering: `clock` and `last` must never be held simultaneously.
    // Always release one before acquiring the other.
    clock: Mutex<ClockFn>,
}

/// `Default` yields `(0, 0)` — the Unix epoch with logical counter zero.
/// This is the earliest valid `HLCTimestamp`, not a sentinel for "uninitialised".
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
        let system_time = self.get_system_clock_time()?;
        // Release clock lock before acquiring `last` — never hold both simultaneously
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

    #[inline]
    fn get_system_clock_time(&self) -> Result<u64, HLCError> {
        let mut sys_clock = self.clock.lock();
        let system_time = (sys_clock)()?;
        drop(sys_clock);
        Ok(system_time)
    }

    #[inline]
    fn increment_logical(t: &HLCTimestamp) -> Result<u16, HLCError> {
        if t.logical < u16::MAX {
            Ok(t.logical + 1u16)
        } else {
            Err(HLCError::LogicalOverflow(*t))
        }
    }

    /// Advances the clock after receiving a message carrying timestamp `ts`.
    ///
    /// Ensures the local clock is strictly greater than both the previous local
    /// timestamp and the received `ts`, maintaining causal ordering across nodes.
    /// Call this on every inbound message before processing it.
    ///
    /// # Errors
    /// - [`HLCError::LogicalOverflow`] if the logical counter would exceed `u16::MAX`.
    pub fn observe(&self, ts: HLCTimestamp) -> Result<HLCTimestamp, HLCError> {
        let system_time = self.get_system_clock_time()?;
        let mut time = self.last.lock();
        let result = match (system_time, time.physical, ts.physical) {
            (st, tt, ts1) if st > tt && st > ts1 => Ok(HLCTimestamp::new(st, 0)),
            (st, tt, ts1) if tt >= st && tt > ts1 => {
                HLC::increment_logical(&time).map(|l| HLCTimestamp::new(tt, l))
            }
            (st, tt, ts1) if ts1 >= st && ts1 > tt => {
                HLC::increment_logical(&ts).map(|l| HLCTimestamp::new(ts1, l))
            }
            (_, tt, ts1) if tt == ts1 => {
                let new_logical = max(time.logical as u32, ts.logical as u32) + 1;
                if new_logical > u16::MAX as u32 {
                    Err(HLCError::LogicalOverflow(HLCTimestamp::new(
                        ts.physical,
                        u16::MAX,
                    )))
                } else {
                    Ok(HLCTimestamp::new(time.physical, new_logical as u16))
                }
            }            
            (_, _, _) => unreachable!("all orderings of (st, tt, ts1) are covered"),
        };
        result.inspect(|&t| *time = t)
    }
}

fn get_epoch_time_now() -> Result<u64, HLCError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|t| t.as_millis() as u64)
        .map_err(HLCError::SystemTimeError)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    impl HLC {
        fn with_clock(clock: ClockFn) -> HLC {
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
    fn test_hlc_timestamp_displays_error() {
        let timestamp = HLCTimestamp::new(977316726400000999, 1);
        assert_eq!(
            format!("{}", timestamp),
            "Invalid time: 977316726400000999.1"
        )
    }

    #[test]
    fn test_hlc_now_increments_logical_for_clock_frozen() {
        let clock = Box::new(|| Ok(1u64));
        let hlc = HLC::with_clock(clock);

        let res1 = hlc.now().unwrap();
        let res2 = hlc.now().unwrap();

        assert!(res2 > res1);
    }

    #[test]
    fn test_hlc_now_increments_logical_from_a_set_state() {
        let clock = Box::new(|| Ok(1u64));
        let hlc = HLC::with_clock(clock);

        hlc.set_time(HLCTimestamp::new(1, 1));
        let res3 = hlc.now().unwrap();

        assert!(res3 > HLCTimestamp::new(1, 1))
    }

    #[test]
    fn test_hlc_now_increasing_monotonically() {
        let mut clock_state: u64 = 1_000_000;
        let clock = Box::new(move || {
            clock_state += 1000;
            Ok(clock_state)
        });
        let hlc = HLC::with_clock(clock);

        let res1 = hlc.now().unwrap();
        let res2 = hlc.now().unwrap();

        assert!(res2 > res1);

        let mut clock_state: u64 = 1_000_000;
        let clock = Box::new(move || {
            clock_state -= 1000;
            Ok(clock_state)
        });
        let hlc = HLC::with_clock(clock);

        let res1 = hlc.now().unwrap();
        let res2 = hlc.now().unwrap();

        assert!(res2 > res1);
    }

    #[test]
    fn test_hlc_now_handles_overflow() {
        let clock = Box::new(|| Ok(1u64));
        let hlc = HLC::with_clock(clock);

        hlc.set_time(HLCTimestamp::new(1, u16::MAX));

        let res = hlc.now();

        assert!(res.is_err());
    }

    #[test]
    fn test_hlc_now_increases_monotonically_in_multithreaded_environment() {
        let mut clock_state: u64 = 1_000_000;
        let clock = Box::new(move || {
            clock_state += 1000;
            Ok(clock_state)
        });
        let hlc = Arc::new(HLC::with_clock(clock));

        let results: Arc<Mutex<Vec<HLCTimestamp>>> = Arc::new(Mutex::new(Vec::new()));

        let handles: Vec<_> = (0..8)
            .map(|_| {
                let hlc = Arc::clone(&hlc);
                let results = Arc::clone(&results);
                std::thread::spawn(move || {
                    for _ in 0..1000 {
                        let ts = hlc.now().unwrap();
                        results.lock().push(ts);
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        let mut all = results.lock().clone();
        all.sort();
        all.dedup();
        assert_eq!(all.len(), 8 * 1000)
    }

    #[test]
    fn test_hlc_observe_increases_monotonically() {
        let clock = Box::new(|| Ok(1000u64));
        let hlc = HLC::with_clock(clock);

        hlc.set_time(HLCTimestamp::new(991, 2));
        let res = hlc.observe(HLCTimestamp::new(998, 1));
        let r_unwrapped = res.unwrap();
        assert_eq!(r_unwrapped, HLCTimestamp::new(1000, 0));
        assert!(hlc.now().unwrap() > r_unwrapped);

        hlc.set_time(HLCTimestamp::new(991, 2));
        let res = hlc.observe(HLCTimestamp::new(1001, 1));
        let r_unwrapped = res.unwrap();
        assert_eq!(r_unwrapped, HLCTimestamp::new(1001, 2));
        assert!(hlc.now().unwrap() > r_unwrapped);

        hlc.set_time(HLCTimestamp::new(1010, 2));
        let res = hlc.observe(HLCTimestamp::new(1001, 1));
        let r_unwrapped = res.unwrap();
        assert_eq!(r_unwrapped, HLCTimestamp::new(1010, 3));
        assert!(hlc.now().unwrap() > r_unwrapped);

        hlc.set_time(HLCTimestamp::new(1000, 2));
        let res = hlc.observe(HLCTimestamp::new(1001, 1));
        let r_unwrapped = res.unwrap();
        assert_eq!(r_unwrapped, HLCTimestamp::new(1001, 2));
        assert!(hlc.now().unwrap() > r_unwrapped);

        hlc.set_time(HLCTimestamp::new(1000, 2));
        let res = hlc.observe(HLCTimestamp::new(998, 1));
        let r_unwrapped = res.unwrap();
        assert_eq!(r_unwrapped, HLCTimestamp::new(1000, 3));
        assert!(hlc.now().unwrap() > r_unwrapped);

        hlc.set_time(HLCTimestamp::new(1000, 2));
        let res = hlc.observe(HLCTimestamp::new(1000, 5));
        let r_unwrapped = res.unwrap();
        assert_eq!(r_unwrapped, HLCTimestamp::new(1000, 6));
        assert!(hlc.now().unwrap() > r_unwrapped);

        let clock = Box::new(|| Ok(500u64));
        let hlc = HLC::with_clock(clock);
        hlc.set_time(HLCTimestamp::new(1000, 2));
        let res = hlc.observe(HLCTimestamp::new(1000, 5));
        let r_unwrapped = res.unwrap();
        assert_eq!(r_unwrapped, HLCTimestamp::new(1000, 6));
        assert!(hlc.now().unwrap() > r_unwrapped);
    }

    #[test]
    fn test_hlc_observer_handles_logical_overflow() {
        let clock = Box::new(|| Ok(1000u64));
        let hlc = HLC::with_clock(clock);

        hlc.set_time(HLCTimestamp::new(1010, u16::MAX));
        let res = hlc.observe(HLCTimestamp::new(1001, 1));
        assert!(res.is_err());

        hlc.set_time(HLCTimestamp::new(991, 2));
        let res = hlc.observe(HLCTimestamp::new(1001, u16::MAX));
        assert!(res.is_err());

        hlc.set_time(HLCTimestamp::new(1000, u16::MAX));
        let res = hlc.observe(HLCTimestamp::new(1000, u16::MAX));
        assert!(res.is_err());

        hlc.set_time(HLCTimestamp::new(1000, u16::MAX));
        let res = hlc.observe(HLCTimestamp::new(998, 1));
        assert!(res.is_err());

        let clock = Box::new(|| Ok(500u64));
        let hlc = HLC::with_clock(clock);
        hlc.set_time(HLCTimestamp::new(1000, u16::MAX));
        let res = hlc.observe(HLCTimestamp::new(1000, u16::MAX));
        assert!(res.is_err());
    }

    #[test]
    fn test_hlc_observe_increases_monotonically_in_multithreaded_environment() {
        let clock = Box::new(|| Ok(1000u64));
        let hlc = Arc::new(HLC::with_clock(clock));
        let results: Arc<Mutex<Vec<HLCTimestamp>>> = Arc::new(Mutex::new(Vec::new()));
        let observes: Arc<Mutex<Vec<HLCTimestamp>>> = Arc::new(Mutex::new(
            (0..(8 * 1000))
                .map(|ph| HLCTimestamp::new(1000u64, ph))
                .collect(),
        ));

        let handles: Vec<_> = (0..8)
            .map(|_| {
                let hlc = Arc::clone(&hlc);
                let results = Arc::clone(&results);
                let observes = Arc::clone(&observes);
                std::thread::spawn(move || {
                    for _ in 0..1000 {
                        let ob = observes.lock().pop().unwrap();
                        let ts = hlc.observe(ob).unwrap();
                        results.lock().push(ts);
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        let mut all = results.lock().clone();
        all.sort();
        all.dedup();
        assert!(all.windows(2).all(|w| w[0] < w[1]));
    }
}
