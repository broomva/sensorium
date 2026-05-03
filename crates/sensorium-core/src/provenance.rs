//! Provenance — the universal `Tagged<T>` wrapper.
//!
//! Every observed value in the substrate is `Tagged<T>` rather than bare `T`.
//! This is the rule from `MIL-PROJECT.md` §10.2:
//!
//! > `Tagged<T>` is the universal currency. Every typed value carries its
//! > confidence, source tokens, and binding kind. Nothing in the contract is
//! > bare.
//!
//! `sensorium-core` is the upstream end of that chain — we tag values *at
//! observation time*, before they ever flow into a directive. Downstream
//! `pneuma-core` re-wraps with its own per-slot bookkeeping; the bridge is
//! a structural conversion.
//!
//! ## What's in a `Provenance`
//!
//! - [`Provenance::sensor`] — which producer emitted the observation.
//! - [`Provenance::observed_at`] — wall-clock time of observation.
//! - [`Provenance::calibration`] — producer's calibration at emission time.
//! - [`Provenance::privacy`] — what privacy tier this value carries.
//! - [`Provenance::primitive`] — which of the seven primitives this value
//!   belongs to.
//!
//! All five are *required* — the substrate refuses to construct a
//! `Provenance` without them. Designing this as a closed struct (rather
//! than an `Option`-of-everything) is intentional: a missing field is a
//! sensor bug, not a runtime branch.

use serde::{Deserialize, Serialize};

use crate::primitive::PrimitiveKind;
use crate::privacy::PrivacyTier;
use crate::sensor::{Calibration, SensorId};
use crate::time::Timestamp;

// --- Provenance --------------------------------------------------------------

/// The audit-trail metadata attached to every observation.
///
/// Five required fields, no optionals. Producers that don't know a field
/// must lie deliberately (e.g. emit `Calibration::synthetic()` rather than
/// dropping the field). This forces honesty in the calibration story.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Provenance {
    /// Producer identity.
    pub sensor: SensorId,
    /// Wall-clock time of observation.
    pub observed_at: Timestamp,
    /// Producer's calibration at emission time.
    pub calibration: Calibration,
    /// Privacy classification of the value.
    pub privacy: PrivacyTier,
    /// Which of the seven MIL primitives this value belongs to. Used by
    /// the `pneuma-router` for primitive-level routing without re-parsing
    /// the value.
    pub primitive: PrimitiveKind,
}

impl Provenance {
    /// Construct a `Provenance`. All five fields are required; we don't
    /// expose `Default` for this type because every field has a real
    /// answer at observation time.
    #[must_use]
    pub fn new(
        sensor: SensorId,
        observed_at: Timestamp,
        calibration: Calibration,
        privacy: PrivacyTier,
        primitive: PrimitiveKind,
    ) -> Self {
        Self {
            sensor,
            observed_at,
            calibration,
            privacy,
            primitive,
        }
    }

    /// `true` if the producer declared its observation calibrated.
    #[must_use]
    pub fn is_calibrated(&self) -> bool {
        self.calibration.is_trusted()
    }
}

// --- Tagged<T> ---------------------------------------------------------------

/// Universal provenance wrapper. Every observed value flows as `Tagged<T>`.
///
/// `Tagged<T>` exists so consumers can pattern-match on the *value* without
/// losing the audit trail. The pattern across the substrate is:
///
/// ```rust,ignore
/// fn handle_fixation(tagged: Tagged<GazeFixation>) {
///     if !tagged.provenance.is_calibrated() {
///         // route through clarification
///         return;
///     }
///     // process tagged.value
/// }
/// ```
///
/// ## Equality semantics
///
/// `PartialEq` compares both value and provenance. To compare *just the
/// value*, dereference: `tagged.value == other.value`.
///
/// ## Serialization
///
/// `Tagged<T>` implements `Serialize` / `Deserialize` iff `T` does. The
/// privacy enforcement happens at the *value* level — wrapping a
/// `LocalOnly<T>` keeps it un-serializable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Tagged<T> {
    /// The observed value.
    pub value: T,
    /// Audit-trail metadata.
    pub provenance: Provenance,
}

impl<T> Tagged<T> {
    /// Construct a tagged value.
    #[must_use]
    pub fn new(value: T, provenance: Provenance) -> Self {
        Self { value, provenance }
    }

    /// Map the inner value, preserving provenance.
    ///
    /// Used when a downstream component refines the value (e.g. resolves a
    /// gaze point to a window) without re-observing it. The provenance
    /// keeps pointing at the original sensor — *the refinement does not
    /// add evidence*.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Tagged<U> {
        Tagged {
            value: f(self.value),
            provenance: self.provenance,
        }
    }

    /// Borrow the value with provenance attached, for ergonomic use in
    /// pattern-match arms.
    #[must_use]
    pub fn as_ref(&self) -> Tagged<&T> {
        Tagged {
            value: &self.value,
            provenance: self.provenance,
        }
    }

    /// `true` if the producer declared its observation calibrated.
    /// Convenience for the most common provenance check.
    #[must_use]
    pub fn is_calibrated(&self) -> bool {
        self.provenance.is_calibrated()
    }
}
