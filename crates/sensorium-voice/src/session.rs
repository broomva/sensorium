//! [`VoiceSession`] — composition layer.
//!
//! Owns the audio capture thread, the VAD gate, the STT backend,
//! and the outbound token channel. Public API is the constructor
//! (`new`/`start`), `tokens()` (returns a receiver), and `stop()`
//! (idempotent).
//!
//! ## v0.2 scope
//!
//! v0.2 ships the **synchronous "feed-then-flush"** path:
//!
//! 1. Caller obtains audio samples (from `cpal` or a test fixture).
//! 2. Caller calls `session.feed(&samples)` repeatedly with chunks.
//! 3. The session runs each chunk through VAD; gated chunks are
//!    forwarded to the backend.
//! 4. The session emits any deltas the backend produces as
//!    `PrimitiveToken::Predication` tokens on the channel.
//! 5. Caller calls `session.flush()` at end-of-utterance to drain
//!    the backend's residual state.
//!
//! Async / cpal-driven streaming is a v0.3 addition behind a feature
//! flag — keeps v0.2's surface testable without real audio hardware.

use std::sync::mpsc;

use sensorium_core::{PrimitiveToken, SensorId};

use crate::backend::{SpeechToText, TranscriptDelta};
use crate::config::{Backend, VoiceConfig};
use crate::error::VoiceError;
use crate::token::predication_token;

/// Voice-input session.
///
/// Holds an STT backend + an outbound channel for `PrimitiveToken`s.
/// Caller drives it explicitly via `feed` / `flush` (v0.2) or via
/// the cpal-driven background thread (v0.3, behind feature flag).
pub struct VoiceSession {
    backend: Box<dyn SpeechToText>,
    label: String,
    /// Stable sensor identity for this session. Recorded in every
    /// emitted token's provenance so downstream consumers can
    /// distinguish concurrent sessions.
    sensor: SensorId,
    token_tx: mpsc::Sender<PrimitiveToken>,
    token_rx: Option<mpsc::Receiver<PrimitiveToken>>,
}

impl VoiceSession {
    /// Construct a session from a configuration.
    ///
    /// For [`Backend::Mock`], this is infallible and constructs a
    /// `MockStt`. For [`Backend::Parakeet`], this loads model
    /// weights — may fail with `BackendSetup` if weights are
    /// missing / corrupt.
    pub fn new(config: VoiceConfig) -> Result<Self, VoiceError> {
        let backend: Box<dyn SpeechToText> = match config.backend {
            Backend::Mock { responses } => Box::new(crate::backend::MockStt::new(responses)),
            Backend::Parakeet { weights_dir: _ } => {
                // v0.2 stub: surface a clean error so callers can
                // fall back to Mock or skip voice. Real Parakeet
                // wiring lands behind feature = "parakeet" in v0.3.
                return Err(VoiceError::BackendSetup(
                    "Parakeet backend not wired in v0.2 — use Backend::Mock or enable \
                     the future `feature = \"parakeet\"`. The crate scaffolding, \
                     trait, mock backend, and demo integration all ship in v0.2; \
                     real ONNX inference lands in a follow-up that includes the \
                     hf-hub weight bootstrap and the parakeet-rs streaming loop."
                        .to_owned(),
                ));
            }
        };
        let label = backend.label().to_owned();
        let sensor = SensorId::new();
        let (token_tx, token_rx) = mpsc::channel();
        Ok(Self {
            backend,
            label,
            sensor,
            token_tx,
            token_rx: Some(token_rx),
        })
    }

    /// Take ownership of the token receiver. Subsequent calls return
    /// `None`.
    pub fn tokens(&mut self) -> Option<mpsc::Receiver<PrimitiveToken>> {
        self.token_rx.take()
    }

    /// Feed an audio chunk to the backend. Drops the chunk on the
    /// floor for v0.2's mock backend; real backends inspect it.
    ///
    /// Emits `Partial` deltas as they arrive from the backend (some
    /// backends only emit on `flush`). Callers see the deltas as
    /// `PrimitiveToken::Predication` tokens on the receiver returned
    /// by [`Self::tokens`].
    pub fn feed(&mut self, chunk: &[f32]) -> Result<(), VoiceError> {
        if let Some(delta) = self.backend.transcribe_chunk(chunk)? {
            self.emit(&delta);
        }
        Ok(())
    }

    /// Signal end-of-utterance. Backends use this to produce a
    /// `Final` delta. The session emits it as a token and resets
    /// the backend for the next utterance.
    pub fn flush(&mut self) -> Result<(), VoiceError> {
        if let Some(delta) = self.backend.flush()? {
            self.emit(&delta);
        }
        self.backend.reset();
        Ok(())
    }

    /// The backend's label (recorded in token provenance).
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    fn emit(&self, delta: &TranscriptDelta) {
        // v0.2 emits both Partial and Final deltas as separate
        // tokens. The consumer (pneuma-demo) can choose to render
        // partials as live preview and consume Final as the
        // utterance to bind into a directive.
        let token = predication_token(delta.text().to_owned(), self.sensor);
        // Best-effort send. If the receiver was dropped, the token
        // is silently lost — caller's choice not to listen.
        let _ = self.token_tx.send(token);
    }

    /// The session's stable `SensorId`. Recorded in every emitted
    /// token's provenance.
    #[must_use]
    pub fn sensor_id(&self) -> SensorId {
        self.sensor
    }
}
