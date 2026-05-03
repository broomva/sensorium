//! The error taxonomy for `sensorium-core`.
//!
//! Errors here represent **substrate-construction failures** — a sensor
//! supplied an invalid identifier, a snapshot identity comparison was
//! attempted across incompatible producers, a ring buffer was constructed
//! at zero capacity. Errors do *not* represent runtime sensor failures
//! (camera occluded, mic muted, gaze tracker miscalibrated) — those live in
//! the producer crates and are surfaced via the `Calibration` /
//! `SensorMetadata::status` fields, not via `Result`.
//!
//! ## Why a closed enum
//!
//! Substrate construction has a small, stable set of failure modes. We use
//! `thiserror` and avoid `Box<dyn Error>` so callers can match on specific
//! variants and so the wire format stays inspectable.

use thiserror::Error;

/// Substrate-construction error.
///
/// Note: `Eq` is intentionally not derived because [`SensoriumError::NotNormalized`]
/// carries an `f32` value (NaN being non-reflexive defeats `Eq`'s contract).
/// Callers that need equality should match on variant discriminants instead.
#[derive(Debug, Clone, PartialEq, Error)]
#[non_exhaustive]
pub enum SensoriumError {
    /// A sensor produced an identifier that was empty or whitespace-only.
    /// This is a sensor bug — the substrate refuses to round-trip it
    /// because it would corrupt downstream equality and indexing.
    #[error("empty or whitespace-only identifier for {field}")]
    EmptyIdentifier {
        /// The struct field whose value was empty.
        field: &'static str,
    },

    /// A [`crate::entity::TextSpan`] was constructed with `end < start`.
    #[error("invalid text span: end ({end}) < start ({start})")]
    InvalidSpan {
        /// Span start byte offset.
        start: u64,
        /// Span end byte offset.
        end: u64,
    },

    /// A [`crate::ring::RingBuffer`] was queried with an out-of-range
    /// capacity or index. Capacity zero is rejected at compile time via
    /// const generics; this variant covers runtime indexing failures.
    #[error("ring buffer index {index} out of range for length {len}")]
    RingIndexOutOfRange {
        /// Requested index.
        index: usize,
        /// Current length.
        len: usize,
    },

    /// A confidence or normalized value was outside its declared `[0.0, 1.0]`
    /// domain. The substrate refuses to construct out-of-domain values
    /// rather than silently clamping, because clamping at the boundary
    /// destroys calibration information.
    #[error("value {value} outside normalized [0.0, 1.0] domain for {field}")]
    NotNormalized {
        /// The struct field whose value was out of range.
        field: &'static str,
        /// The offending value.
        value: f32,
    },

    /// A privacy operation was attempted that would expose protected data.
    /// E.g. attempting to serialize a [`crate::privacy::LocalOnly`] without
    /// declassification.
    #[error("privacy violation: {reason}")]
    PrivacyViolation {
        /// Human-readable reason — what the caller tried to do, in passive voice.
        reason: &'static str,
    },
}

/// Convenient `Result` alias for substrate operations.
pub type Result<T, E = SensoriumError> = core::result::Result<T, E>;
