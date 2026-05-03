//! Posture and presence snapshot.
//!
//! "Is the user there? Are they leaning forward? Is the device on their
//! lap or on a desk?" — coarse signals that inform Autonomic's
//! threshold-adjustment policy alongside biometrics. Producers:
//! `sensorium-vision` (camera-based posture detection),
//! `sensorium-headset` (IMU on the device).
//!
//! Like biometrics, posture data is [`crate::PrivacyTier::Sensitive`] and
//! must be wrapped in [`crate::LocalOnly`] before crossing crate
//! boundaries.

use serde::{Deserialize, Serialize};

use crate::error::{Result, SensoriumError};
use crate::time::Timestamp;

// --- PresenceLevel -----------------------------------------------------------

/// Whether the user is present at the device.
///
/// Coarse buckets — presence detection fuses ambient light, face
/// detection, and IMU activity, none of which is exact. The substrate
/// uses `Unknown` rather than fabricating a default when no producer
/// can decide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PresenceLevel {
    /// User is at the device, actively engaged.
    Present,
    /// User is at the device but not actively engaged (idle).
    Idle,
    /// User has stepped away. The device is on but the seat is empty.
    Absent,
    /// No producer can decide. Treat as if a sensor is failed.
    Unknown,
}

impl PresenceLevel {
    /// `true` when the user is actively engaged. Pneuma may proceed with
    /// optimistic dispatch; otherwise it should default to ratification.
    #[must_use]
    pub fn is_engaged(self) -> bool {
        matches!(self, Self::Present)
    }
}

// --- Posture -----------------------------------------------------------------

/// Coarse posture classification.
///
/// Used by Autonomic alongside biometrics to estimate "how committed is
/// the user to the current task?" — leaning back is associated with
/// reflection / browsing; leaning forward with intent / focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Posture {
    /// User leaning toward the screen — focused / engaged / committed.
    LeanForward,
    /// User in a neutral upright posture.
    Upright,
    /// User leaning back / relaxed.
    LeanBack,
    /// Posture indicates fatigue: slumped, head supported, etc.
    Fatigued,
    /// No producer can decide.
    Unknown,
}

impl Posture {
    /// `true` if the producer believes the user is fatigued. Autonomic
    /// should *tighten* confidence thresholds here, not loosen them.
    #[must_use]
    pub fn indicates_fatigue(self) -> bool {
        matches!(self, Self::Fatigued)
    }
}

// --- PostureSnapshot ---------------------------------------------------------

/// A point-in-time posture snapshot.
///
/// `face_distance_cm` is optional — many camera-only producers can detect
/// presence and posture but cannot measure distance reliably. When
/// available it's a useful continuous signal alongside the discrete
/// `Posture` bucket.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PostureSnapshot {
    /// Discrete posture classification.
    pub posture: Posture,
    /// Discrete presence classification.
    pub presence: PresenceLevel,
    /// Estimated face-to-screen distance in centimeters. Validated as
    /// positive at construction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face_distance_cm: Option<f32>,
    /// When this snapshot was taken.
    pub at: Timestamp,
}

impl PostureSnapshot {
    /// Construct, validating that `face_distance_cm` (if present) is
    /// positive.
    ///
    /// Rejects `NaN`, zero, and negative distances — all sensor bugs.
    pub fn new(
        posture: Posture,
        presence: PresenceLevel,
        face_distance_cm: Option<f32>,
        at: Timestamp,
    ) -> Result<Self> {
        if let Some(d) = face_distance_cm
            && (d.is_nan() || d <= 0.0)
        {
            return Err(SensoriumError::NotNormalized {
                field: "PostureSnapshot.face_distance_cm",
                value: d,
            });
        }
        Ok(Self {
            posture,
            presence,
            face_distance_cm,
            at,
        })
    }

    /// A neutral default snapshot — `Unknown` everywhere. Used as the
    /// starting state when no posture producer is present.
    #[must_use]
    pub fn unknown(at: Timestamp) -> Self {
        Self {
            posture: Posture::Unknown,
            presence: PresenceLevel::Unknown,
            face_distance_cm: None,
            at,
        }
    }
}
