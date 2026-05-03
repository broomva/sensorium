//! Biometric snapshot types.
//!
//! Heart-rate, skin conductance, and a derived [`ArousalLevel`] enum. These
//! feed Autonomic, which adjusts Pneuma's confidence threshold based on
//! observed user state (`MIL-PROJECT.md` §7.4):
//!
//! > Autonomic over Pneuma. The router's confidence threshold is a function
//! > of the agent's `OperatingMode`. Verify mode → stricter; Execute mode →
//! > relaxed; Recover mode → blocks non-deterministic dispatch.
//!
//! Biometric values are [`crate::PrivacyTier::Sensitive`] by hard policy and
//! must be wrapped in [`crate::LocalOnly`] before they leave this crate's
//! API surface.
//!
//! ## Numeric domains
//!
//! - `heart_rate_bpm`: positive, typically 30–220 BPM. Producers that emit
//!   physically-impossible values are buggy and the substrate refuses
//!   them.
//! - `skin_conductance_us`: positive microsiemens. Range varies by
//!   sensor; we don't enforce an upper bound.

use serde::{Deserialize, Serialize};

use crate::error::{Result, SensoriumError};
use crate::time::Timestamp;

// --- HeartRate ---------------------------------------------------------------

/// Beats-per-minute heart rate.
///
/// Validated at construction: must be in `(0, 300)` BPM. Anything outside
/// that range is a sensor bug.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HeartRate(f32);

impl HeartRate {
    /// Construct, validating the BPM is in `(0.0, 300.0)`.
    ///
    /// Rejects `NaN` and the open interval bounds (zero and 300+ are sensor
    /// bugs). The check uses explicit `<=`/`>=` plus an `is_nan` guard
    /// rather than `!(0.0 < bpm && bpm < 300.0)` so the intent reads
    /// directly.
    pub fn new(bpm: f32) -> Result<Self> {
        if bpm.is_nan() || bpm <= 0.0 || bpm >= 300.0 {
            return Err(SensoriumError::NotNormalized {
                field: "HeartRate.bpm",
                value: bpm,
            });
        }
        Ok(Self(bpm))
    }

    /// BPM value.
    #[must_use]
    pub fn bpm(self) -> f32 {
        self.0
    }
}

// --- SkinConductance ---------------------------------------------------------

/// Skin conductance / GSR, in microsiemens.
///
/// Validated as positive at construction.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SkinConductance(f32);

impl SkinConductance {
    /// Construct, validating the value is positive (µS > 0).
    ///
    /// Rejects `NaN`, zero, and negatives — all sensor bugs. Explicit
    /// comparison rather than `!(us > 0.0)` so the intent reads directly.
    pub fn new(us: f32) -> Result<Self> {
        if us.is_nan() || us <= 0.0 {
            return Err(SensoriumError::NotNormalized {
                field: "SkinConductance.us",
                value: us,
            });
        }
        Ok(Self(us))
    }

    /// Microsiemens value.
    #[must_use]
    pub fn microsiemens(self) -> f32 {
        self.0
    }
}

// --- ArousalLevel ------------------------------------------------------------

/// A coarse arousal estimate derived from biometric streams.
///
/// Producers compute this from the underlying signals (HR variability,
/// GSR, breathing rate) and pass *the estimate*, not the raw values, to
/// downstream consumers that don't need the privacy-burdened raw stream.
/// This keeps Autonomic's threshold-adjustment policy loose-coupled to
/// the specific biometric hardware.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ArousalLevel {
    /// Calm / well-rested / unhurried.
    Low,
    /// Normal working state. Default when nothing else applies.
    Normal,
    /// Heightened — focused or mildly stressed. Confidence thresholds
    /// should *tighten* here per the safety-asymmetry rule (urgency does
    /// not lower thresholds).
    Elevated,
    /// Acute — distressed or strongly hurried. Thresholds tighten further;
    /// destructive actions should require longer ratification dwell.
    Acute,
}

impl ArousalLevel {
    /// `true` if the level indicates distress or strong stress. Confidence
    /// thresholds and ratification dwell should tighten when this is `true`.
    #[must_use]
    pub fn is_high_arousal(self) -> bool {
        matches!(self, Self::Elevated | Self::Acute)
    }
}

// --- BiometricSnapshot -------------------------------------------------------

/// A point-in-time biometric snapshot.
///
/// All optional — a session may have no biometric sensor at all (`HeartRate`,
/// `SkinConductance` both `None`) but still produce an `ArousalLevel`
/// inferred from typing rhythm or other proxies.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BiometricSnapshot {
    /// Heart rate, when measured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heart_rate: Option<HeartRate>,
    /// Skin conductance, when measured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skin_conductance: Option<SkinConductance>,
    /// Producer's arousal estimate. Required even when the underlying
    /// signals are unavailable — producers fall back to `ArousalLevel::Normal`.
    pub arousal: ArousalLevel,
    /// When this snapshot was taken.
    pub at: Timestamp,
}

impl BiometricSnapshot {
    /// A neutral baseline snapshot — no biometrics available, arousal
    /// `Normal`. Used as the starting state for sessions without any
    /// biometric producer.
    #[must_use]
    pub fn neutral(at: Timestamp) -> Self {
        Self {
            heart_rate: None,
            skin_conductance: None,
            arousal: ArousalLevel::Normal,
            at,
        }
    }
}
