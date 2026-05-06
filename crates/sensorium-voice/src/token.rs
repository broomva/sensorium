//! `PrimitiveToken` construction helpers.
//!
//! Voice tokens are `PrimitiveToken::Predication(Tagged<String>)` —
//! the open-ended utterance content carrying the full transcribed
//! text plus provenance.

use sensorium_core::{
    Calibration, PrimitiveKind, PrimitiveToken, PrivacyTier, Provenance, SensorId, Tagged,
    Timestamp,
};

/// Construct a `PrimitiveToken::Predication` from raw transcript text
/// plus the producing session's `SensorId`.
///
/// The SensorId (UUID) is recorded in provenance so downstream
/// consumers can distinguish voice-sourced predications from
/// keyboard-typed ones, audio dictation from a remote relay, etc.
/// The backend's human-readable label (e.g., `"parakeet-eou"`,
/// `"mock"`) is recorded separately by the session in its journal
/// entries and HUD frames.
///
/// Privacy tier is `Private` — voice transcripts are local-by-default
/// (may be journaled but never forwarded without explicit consent).
/// Calibration is `uncalibrated()` since the model has no
/// session-level calibration step in v0.2.
#[must_use]
pub fn predication_token(text: String, sensor: SensorId) -> PrimitiveToken {
    let provenance = Provenance::new(
        sensor,
        Timestamp::now(),
        Calibration::uncalibrated(),
        PrivacyTier::Private,
        PrimitiveKind::Predication,
    );
    PrimitiveToken::Predication(Tagged::new(text, provenance))
}
