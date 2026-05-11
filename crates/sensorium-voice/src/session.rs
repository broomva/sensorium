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

use sensorium_core::{Generation, GenerationSeq, PrimitiveToken, SensorId, StreamUpdate};

use crate::backend::{SpeechToText, TranscriptDelta};
use crate::config::{Backend, VoiceConfig};
use crate::error::VoiceError;
use crate::token::predication_token;
use crate::vad::{VadEvent, VadGate, VadModel};

/// Voice-input session.
///
/// Holds an STT backend, two outbound channels — the legacy
/// `PrimitiveToken::Predication` stream for callers that just want
/// the text, and the generation-tagged `StreamUpdate<TranscriptDelta>`
/// stream for downstream stages that compose on speculative
/// generations — and a per-session monotonic [`GenerationSeq`].
///
/// One utterance corresponds to one generation. The session mints a
/// fresh generation on the first delta of an utterance (on the first
/// `feed()` call after construction, after `flush()`, or after
/// `cancel()`), and emits `StreamUpdate::Final` (or `Cancelled`) with
/// that generation when the utterance closes.
pub struct VoiceSession {
    backend: Box<dyn SpeechToText>,
    label: String,
    /// Stable sensor identity for this session. Recorded in every
    /// emitted token's provenance so downstream consumers can
    /// distinguish concurrent sessions.
    sensor: SensorId,
    /// Legacy `PrimitiveToken` channel (kept for backward compat with
    /// callers like `pneuma-demo` that consume bare text).
    token_tx: mpsc::Sender<PrimitiveToken>,
    token_rx: Option<mpsc::Receiver<PrimitiveToken>>,
    /// Generation-tagged streaming channel — the substrate primitive
    /// downstream stages compose on.
    stream_tx: mpsc::Sender<StreamUpdate<TranscriptDelta>>,
    stream_rx: Option<mpsc::Receiver<StreamUpdate<TranscriptDelta>>>,
    /// Monotonic generation minter for this session.
    gen_seq: GenerationSeq,
    /// The generation owning the *current* utterance, if any. `None`
    /// when the session is idle (before first feed, or after
    /// flush/cancel). Minted lazily on the first delta of an
    /// utterance.
    current_generation: Option<Generation>,
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
            #[cfg(feature = "parakeet")]
            Backend::Parakeet { weights_dir } => {
                Box::new(crate::parakeet::ParakeetStt::new(weights_dir)?)
            }
            #[cfg(not(feature = "parakeet"))]
            Backend::Parakeet { weights_dir: _ } => {
                return Err(VoiceError::BackendSetup(
                    "Parakeet backend requires `feature = \"parakeet\"`. \
                     Build with `cargo build --features parakeet` (or add it \
                     to your dependent crate's feature list). The Parakeet \
                     path adds parakeet-rs + ort + hf-hub deps; the default \
                     build stays dep-light."
                        .to_owned(),
                ));
            }
        };
        let label = backend.label().to_owned();
        let sensor = SensorId::new();
        let (token_tx, token_rx) = mpsc::channel();
        let (stream_tx, stream_rx) = mpsc::channel();
        Ok(Self {
            backend,
            label,
            sensor,
            token_tx,
            token_rx: Some(token_rx),
            stream_tx,
            stream_rx: Some(stream_rx),
            gen_seq: GenerationSeq::new(),
            current_generation: None,
        })
    }

    /// Take ownership of the legacy `PrimitiveToken` receiver.
    /// Subsequent calls return `None`.
    ///
    /// Emits `PrimitiveToken::Predication` for every `Partial` /
    /// `Final` delta from the backend — same shape as before B2.
    /// Callers that only need bare text continue to use this; callers
    /// that need generation tagging use [`Self::streaming_tokens`].
    pub fn tokens(&mut self) -> Option<mpsc::Receiver<PrimitiveToken>> {
        self.token_rx.take()
    }

    /// Take ownership of the generation-tagged `StreamUpdate` receiver.
    /// Subsequent calls return `None`.
    ///
    /// Emits `StreamUpdate<TranscriptDelta>` for every delta the
    /// backend produces, with the generation owning the current
    /// utterance. A `Partial` delta surfaces as
    /// `StreamUpdate::Partial`; a `Final` delta as `StreamUpdate::Final`;
    /// calling [`Self::cancel`] surfaces `StreamUpdate::Cancelled`.
    ///
    /// Downstream stages (resolver, Arcan, TTS) compose on this
    /// channel — they tag derived work with the generation and can
    /// cleanly drop it on `Cancelled` or supersession.
    pub fn streaming_tokens(&mut self) -> Option<mpsc::Receiver<StreamUpdate<TranscriptDelta>>> {
        self.stream_rx.take()
    }

    /// The generation owning the current utterance, if any.
    ///
    /// `None` when the session is idle (before first feed, after
    /// flush, or after cancel). `Some(g)` between the first delta of
    /// an utterance and the `Final` / `Cancelled` that closes it.
    #[must_use]
    pub fn current_generation(&self) -> Option<Generation> {
        self.current_generation
    }

    /// Feed an audio chunk to the backend. Drops the chunk on the
    /// floor for the mock backend; real backends inspect it.
    ///
    /// Emits `Partial` deltas as they arrive from the backend (some
    /// backends only emit on `flush`). Callers see the deltas as
    /// `PrimitiveToken::Predication` tokens on the legacy channel
    /// [`Self::tokens`], AND as `StreamUpdate::Partial` updates on the
    /// streaming channel [`Self::streaming_tokens`].
    ///
    /// The first `feed()` of an utterance mints a fresh generation
    /// from the session's [`GenerationSeq`]; subsequent feeds in the
    /// same utterance reuse it until `flush()` or `cancel()` closes
    /// the utterance.
    pub fn feed(&mut self, chunk: &[f32]) -> Result<(), VoiceError> {
        if let Some(delta) = self.backend.transcribe_chunk(chunk)? {
            let generation = self.ensure_generation();
            self.emit_legacy(&delta);
            self.emit_streaming(StreamUpdate::Partial {
                generation,
                value: delta,
            });
        } else {
            // Even when the backend emits nothing, the first feed of
            // an utterance establishes the generation so `cancel()` /
            // `current_generation()` see the right value.
            let _ = self.ensure_generation();
        }
        Ok(())
    }

    /// Signal end-of-utterance. Backends use this to produce a
    /// `Final` delta. The session emits it on both channels, resets
    /// the backend, and clears the current generation.
    ///
    /// If no generation has been minted yet (no prior `feed`), this
    /// flush will still mint one for the `Final` delta — the
    /// utterance was zero-duration but real.
    pub fn flush(&mut self) -> Result<(), VoiceError> {
        if let Some(delta) = self.backend.flush()? {
            let generation = self.ensure_generation();
            self.emit_legacy(&delta);
            self.emit_streaming(StreamUpdate::Final {
                generation,
                value: delta,
            });
        }
        self.backend.reset();
        self.current_generation = None;
        Ok(())
    }

    /// Abandon the current utterance — barge-in path.
    ///
    /// If a generation is active, emits
    /// `StreamUpdate::Cancelled { generation }` so downstream stages
    /// can drop pending speculative work tagged with it. Resets the
    /// backend and clears the active generation.
    ///
    /// Idempotent / no-op when the session is idle.
    pub fn cancel(&mut self) -> Result<(), VoiceError> {
        if let Some(generation) = self.current_generation.take() {
            self.emit_streaming(StreamUpdate::Cancelled { generation });
            self.backend.reset();
        }
        Ok(())
    }

    /// The backend's label (recorded in token provenance).
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Mint a generation lazily on first delta of an utterance.
    fn ensure_generation(&mut self) -> Generation {
        if let Some(g) = self.current_generation {
            return g;
        }
        let g = self.gen_seq.advance();
        self.current_generation = Some(g);
        g
    }

    fn emit_legacy(&self, delta: &TranscriptDelta) {
        // Legacy contract: emit Partial and Final deltas as separate
        // tokens. The consumer (pneuma-demo) renders partials as live
        // preview and consumes Final as the utterance to bind into a
        // directive.
        let token = predication_token(delta.text().to_owned(), self.sensor);
        // Best-effort send. If the receiver was dropped, the token
        // is silently lost — caller's choice not to listen.
        let _ = self.token_tx.send(token);
    }

    fn emit_streaming(&self, update: StreamUpdate<TranscriptDelta>) {
        // Best-effort send. Same drop semantics as the legacy channel.
        let _ = self.stream_tx.send(update);
    }

    /// The session's stable `SensorId`. Recorded in every emitted
    /// token's provenance.
    #[must_use]
    pub fn sensor_id(&self) -> SensorId {
        self.sensor
    }

    /// Drive a stream of audio chunks through a VAD model + the
    /// session's STT backend.
    ///
    /// `samples` is a mono `f32` stream at the VAD's required
    /// sample rate (16kHz for `EnergyVad` and the future Silero V5
    /// re-introduction). The driver:
    ///
    /// 1. Buffers samples to the VAD's chunk size (512 by default).
    /// 2. Calls `vad.predict(chunk)` to get per-chunk speech
    ///    probability.
    /// 3. Feeds the chunk into [`VadGate::observe`] — the gate
    ///    transitions Idle → Speaking on hysteresis-confirmed onset
    ///    and Speaking → Idle on hysteresis-confirmed offset.
    /// 4. While Speaking, forwards audio chunks to the STT backend
    ///    via `feed(chunk)`. Partials emerge from the channel as
    ///    they're produced.
    /// 5. On `SpeechEnd`, calls `flush()` to surface the Final
    ///    delta, then resets backend state for the next utterance.
    ///
    /// `samples` may be a finite slice (test fixture) or a streaming
    /// iterator drained from a `cpal` ringbuf consumer. Call
    /// `flush()` after the iterator drains to surface any tail
    /// utterance the gate hadn't closed.
    ///
    /// Returns the number of completed utterances (SpeechEnd events
    /// observed) for diagnostics.
    pub fn run_vad_driven<I, V>(
        &mut self,
        samples: I,
        vad: &mut V,
        gate: &mut VadGate,
    ) -> Result<u32, VoiceError>
    where
        I: IntoIterator<Item = f32>,
        V: VadModel + ?Sized,
    {
        let chunk_size = vad.chunk_size();
        let mut buffer: Vec<f32> = Vec::with_capacity(chunk_size);
        let mut utterances = 0_u32;

        for sample in samples {
            buffer.push(sample);
            if buffer.len() < chunk_size {
                continue;
            }
            let probability = vad.predict(&buffer)?;

            // Forward audio to the backend whenever the gate is in
            // Speaking. We forward BEFORE observing the new chunk so
            // a SpeechStart-causing chunk also reaches the backend.
            let was_speaking = gate.is_speaking();
            if was_speaking {
                self.feed(&buffer)?;
            }
            match gate.observe(probability) {
                Some(VadEvent::SpeechStart) => {
                    // Forward the chunk that crossed the threshold;
                    // it carries the onset of speech.
                    self.feed(&buffer)?;
                }
                Some(VadEvent::SpeechEnd) => {
                    // Drain the backend; emit the Final delta.
                    self.flush()?;
                    utterances += 1;
                }
                None => {}
            }
            buffer.clear();
        }

        Ok(utterances)
    }
}
