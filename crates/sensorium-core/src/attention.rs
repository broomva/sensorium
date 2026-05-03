//! Gaze and attention types.
//!
//! Producers: webcam-based eye trackers (`sensorium-gaze`), Tobii hardware,
//! or headset-resident gaze (`sensorium-headset`). Consumers: the
//! `pneuma-resolver` (gaze ↔ workspace hit-test), `pneuma-binder`
//! (cross-modal binding of deictics).
//!
//! The data shapes here are deliberately unitless and screen-space; the
//! producer is responsible for de-bouncing, smoothing, and re-projecting.
//! What `sensorium-core` cares about is having a clean substrate boundary.
//!
//! ## Privacy
//!
//! Gaze fixations are [`crate::PrivacyTier::Sensitive`] by default. Producers
//! must wrap raw fixation streams in [`crate::LocalOnly`] before exposing
//! them outside this crate. The substrate provides the wrapping; the
//! enforcement is in the type system.

use serde::{Deserialize, Serialize};

use crate::error::{Result, SensoriumError};
use crate::time::Timestamp;

// --- GazePoint ---------------------------------------------------------------

/// A single gaze sample in screen-space pixel coordinates.
///
/// Coordinates use the OS convention (top-left origin, X right, Y down).
/// Values are `f32` rather than `i32` so subpixel smoothing survives.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GazePoint {
    /// Screen-space X in pixels.
    pub x: f32,
    /// Screen-space Y in pixels.
    pub y: f32,
}

impl GazePoint {
    /// Construct a gaze point. No validation — gaze trackers may legitimately
    /// emit off-screen points (the user looked off the monitor) and the
    /// substrate preserves them for the resolver to handle.
    #[must_use]
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

// --- GazeSample --------------------------------------------------------------

/// A timestamped gaze sample with confidence in `[0.0, 1.0]`.
///
/// `confidence` is the producer's per-sample reliability — distinct from
/// the producer's overall calibration. A well-calibrated tracker may
/// emit a low-confidence sample when the user blinks; the producer is
/// trusted, but this individual sample is not.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GazeSample {
    /// Where the user is looking.
    pub point: GazePoint,
    /// Per-sample confidence in `[0.0, 1.0]`. Construction validates the
    /// range; values outside it are a sensor bug.
    pub confidence: f32,
    /// Sample timestamp.
    pub at: Timestamp,
}

impl GazeSample {
    /// Construct a gaze sample, validating that `confidence` is in
    /// `[0.0, 1.0]`.
    pub fn new(point: GazePoint, confidence: f32, at: Timestamp) -> Result<Self> {
        if !(0.0..=1.0).contains(&confidence) {
            return Err(SensoriumError::NotNormalized {
                field: "GazeSample.confidence",
                value: confidence,
            });
        }
        Ok(Self {
            point,
            confidence,
            at,
        })
    }
}

// --- Fixation ----------------------------------------------------------------

/// A detected fixation — the user held their gaze on a region of screen.
///
/// Fixations are the *productive* output of a gaze tracker for the
/// substrate's purposes. Raw saccades are noise; fixations are intent.
/// The producer detects fixations via dwell-time threshold (typically
/// 200–600 ms) and emits this struct.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Fixation {
    /// Mean gaze point during the fixation, screen-space pixels.
    pub center: GazePoint,
    /// Duration the user held this fixation, milliseconds.
    pub dwell_ms: u32,
    /// When the fixation *started*. End time is `start + dwell_ms`.
    pub started_at: Timestamp,
}

impl Fixation {
    /// Construct a fixation.
    #[must_use]
    pub fn new(center: GazePoint, dwell_ms: u32, started_at: Timestamp) -> Self {
        Self {
            center,
            dwell_ms,
            started_at,
        }
    }
}

// --- GazeFixation (the rich event variant) -----------------------------------

/// A fixation augmented with optional resolved hit-target metadata.
///
/// Most consumers want both the screen-space fixation *and* whatever the
/// substrate already resolved from that fixation (which window? which
/// file? which selection?). The resolved target is `Option` because the
/// gaze producer typically does not have access to the workspace
/// observer; resolution happens later in the pipeline. When present, it
/// short-circuits a hit-test that the resolver would otherwise repeat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GazeFixation {
    /// The underlying fixation.
    pub fixation: Fixation,
    /// Resolved target ID, if hit-tested. The resolver fills this in
    /// downstream; gaze producers typically leave it `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_target: Option<String>,
}

impl GazeFixation {
    /// Construct from a [`Fixation`] with no resolved target.
    #[must_use]
    pub fn new(fixation: Fixation) -> Self {
        Self {
            fixation,
            resolved_target: None,
        }
    }

    /// Attach a resolved target identifier.
    #[must_use]
    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.resolved_target = Some(target.into());
        self
    }
}
