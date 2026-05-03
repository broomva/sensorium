//! Time types used across the substrate.
//!
//! Two time systems coexist on purpose:
//!
//! - [`Timestamp`] — wall-clock UTC. Stable across processes and machines.
//!   Used in serialized records (snapshot creation time, journal entries).
//!   Not monotonic; can move backwards under NTP sync.
//!
//! - [`Monotonic`] — process-local monotonic clock. Used for *durations* and
//!   *ordering* within a session. Cannot be serialized across processes
//!   because the epoch is the process start. Used by the ring buffer to
//!   compute "recency" without trusting wall-clock.
//!
//! The substrate uses both: timestamps for identity and durability, monotonic
//! for in-flight reasoning. Producers stamp tokens with both.

use std::time::Instant;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A wall-clock UTC timestamp, serializable.
///
/// Wraps `chrono::DateTime<Utc>`. The newtype exists so we can change the
/// backing implementation later (e.g. to `jiff` or a `i64` nanos value)
/// without rippling through every consumer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Timestamp(DateTime<Utc>);

impl Timestamp {
    /// Capture the current wall-clock time.
    #[must_use]
    pub fn now() -> Self {
        Self(Utc::now())
    }

    /// Construct from milliseconds since the Unix epoch.
    ///
    /// Returns `None` if the value is out of range for `DateTime<Utc>`.
    #[must_use]
    pub fn from_millis_utc(millis: i64) -> Option<Self> {
        DateTime::<Utc>::from_timestamp_millis(millis).map(Self)
    }

    /// Milliseconds since the Unix epoch.
    #[must_use]
    pub fn as_millis_utc(self) -> i64 {
        self.0.timestamp_millis()
    }

    /// Inner `chrono` value, for callers that want full date-time arithmetic.
    #[must_use]
    pub fn into_inner(self) -> DateTime<Utc> {
        self.0
    }
}

impl From<DateTime<Utc>> for Timestamp {
    fn from(value: DateTime<Utc>) -> Self {
        Self(value)
    }
}

/// A process-local monotonic instant.
///
/// Cannot be serialized: the epoch is the process start. Use [`Timestamp`] for
/// anything that crosses a process boundary.
#[derive(Debug, Clone, Copy)]
pub struct Monotonic(Instant);

impl Monotonic {
    /// Capture the current monotonic instant.
    #[must_use]
    pub fn now() -> Self {
        Self(Instant::now())
    }

    /// Duration since `earlier`, saturating at zero if `earlier` is in the
    /// future (i.e. caller mixed up the order).
    #[must_use]
    pub fn since(self, earlier: Self) -> std::time::Duration {
        self.0.saturating_duration_since(earlier.0)
    }

    /// Inner `Instant`, for callers that want `std::time` arithmetic.
    #[must_use]
    pub fn into_inner(self) -> Instant {
        self.0
    }
}

impl PartialEq for Monotonic {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for Monotonic {}

impl PartialOrd for Monotonic {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Monotonic {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}
