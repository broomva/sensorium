//! Aggregated user state — what Autonomic reads when it adjusts thresholds.
//!
//! [`UserState`] is the *summary* of biometric, posture, and attention
//! channels at a point in time, plus a derived [`CognitiveLoad`] estimate.
//! Pneuma uses this to call into [`pneuma_core::PolicyEnvelope::tighten_by_state`]
//! (the `tightened_by_state` flag from `MIL-PROJECT.md` §6.1).
//!
//! ## The asymmetry, made structural
//!
//! From the spec: *urgency does not lower the confidence threshold; only
//! shortens ratify dwell. Carefulness raises thresholds.* The substrate
//! does not enforce that asymmetry — Pneuma does — but this module
//! defines the *signals* Pneuma reads to make the call. If a sensor
//! reports `CognitiveLoad::Overloaded`, Pneuma should *tighten*, not
//! loosen.

use serde::{Deserialize, Serialize};

use crate::biometric::BiometricSnapshot;
use crate::posture::PostureSnapshot;
use crate::time::Timestamp;

// --- CognitiveLoad -----------------------------------------------------------

/// A coarse estimate of cognitive load, derived from biometric +
/// posture + activity rate.
///
/// The asymmetry the substrate is designed around: *higher load → tighter
/// thresholds*. The variants are ordered to make this monotonic — `Underloaded
/// < Nominal < Engaged < Overloaded`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum CognitiveLoad {
    /// User is barely engaged — idle, distracted, or just starting a session.
    Underloaded,
    /// Default working state.
    Nominal,
    /// Focused, productive — biometrics elevated but posture forward, fluent
    /// activity rate.
    Engaged,
    /// Overloaded — high arousal, fatigue posture, erratic activity. Pneuma
    /// should *tighten* thresholds and prefer clarification over guessing.
    Overloaded,
}

impl CognitiveLoad {
    /// `true` if the load level should cause Pneuma to tighten thresholds.
    /// Both `Engaged` and `Overloaded` qualify — engaged because the user
    /// is committed and a misfire is more costly; overloaded because the
    /// user is stressed and a misfire is harder to recover from.
    #[must_use]
    pub fn should_tighten_threshold(self) -> bool {
        matches!(self, Self::Engaged | Self::Overloaded)
    }
}

// --- UserState ---------------------------------------------------------------

/// Aggregated point-in-time user state. The substrate's read-only summary
/// for Autonomic.
///
/// Composed from the upstream snapshots — biometric, posture — plus a
/// derived cognitive-load estimate. Producers fill in whatever they have;
/// missing components default to neutral / unknown values.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct UserState {
    /// Biometric snapshot. Always present (use `BiometricSnapshot::neutral`
    /// when no biometric sensor exists).
    pub biometric: BiometricSnapshot,
    /// Posture snapshot. Always present (use `PostureSnapshot::unknown`
    /// when no posture producer exists).
    pub posture: PostureSnapshot,
    /// Derived cognitive-load estimate.
    pub cognitive_load: CognitiveLoad,
    /// When this state was assembled. May post-date the underlying
    /// snapshots by a few hundred ms.
    pub at: Timestamp,
}

impl UserState {
    /// A neutral default state — no biometric, posture unknown, load
    /// nominal. Used as the starting state at session-start before any
    /// producer has reported.
    #[must_use]
    pub fn neutral(at: Timestamp) -> Self {
        Self {
            biometric: BiometricSnapshot::neutral(at),
            posture: PostureSnapshot::unknown(at),
            cognitive_load: CognitiveLoad::Nominal,
            at,
        }
    }

    /// `true` if the substrate believes Pneuma should tighten thresholds.
    /// Combines arousal, fatigue, and cognitive-load signals.
    #[must_use]
    pub fn should_tighten_threshold(&self) -> bool {
        self.cognitive_load.should_tighten_threshold()
            || self.biometric.arousal.is_high_arousal()
            || self.posture.posture.indicates_fatigue()
    }
}
