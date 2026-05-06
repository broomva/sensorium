//! Voice Activity Detection (VAD) abstraction.
//!
//! Two-piece design:
//!
//! 1. [`VadModel`] trait — abstracts over the VAD engine. Concrete
//!    impls: [`MockVad`] (no deps, programmable for tests),
//!    `SileroVad` (cfg-gated to `feature = "audio"`, wraps Silero V5
//!    via the `voice_activity_detector` crate).
//! 2. [`VadGate`] — utterance-boundary state machine. Consumes a
//!    stream of probabilities and emits `VadEvent::SpeechStart` /
//!    `SpeechEnd` based on configurable hysteresis thresholds.
//!
//! The model is the cheap, stateless part (probability per chunk).
//! The gate carries the meaningful state — speech onset, hangover
//! counting, configurable thresholds. Splitting them lets the gate
//! be tested with any model, including the Mock.

use crate::error::VoiceError;

/// Per-chunk VAD model surface.
///
/// Implementations consume a fixed-size chunk of audio samples and
/// return a probability of speech in `[0.0, 1.0]`.
///
/// Silero V5 (the engine we wrap in `SileroVad`) requires fixed
/// chunk sizes: 256 samples at 8kHz, 512 samples at 16kHz. Other
/// implementations may have different constraints.
pub trait VadModel: Send {
    /// Predict speech probability for one chunk of `f32` samples
    /// at the configured sample rate.
    fn predict(&mut self, samples: &[f32]) -> Result<f32, VoiceError>;

    /// Sample rate the model expects. Used by the gate to size
    /// audio buffers correctly.
    fn sample_rate(&self) -> u32;

    /// Required chunk size in samples. The gate buffers up to this
    /// size before each `predict` call.
    fn chunk_size(&self) -> usize;
}

// --- MockVad --------------------------------------------------------------

/// Programmable VAD for tests.
///
/// Holds a queue of probabilities and emits each one on `predict`.
/// When the queue drains, repeats the last probability — useful for
/// tests that drive the gate longer than they primed.
#[derive(Debug, Clone)]
pub struct MockVad {
    queue: std::collections::VecDeque<f32>,
    last: f32,
    sample_rate: u32,
    chunk_size: usize,
}

impl MockVad {
    /// Construct from a sequence of probabilities.
    #[must_use]
    pub fn new(probabilities: impl IntoIterator<Item = f32>) -> Self {
        let queue: std::collections::VecDeque<f32> = probabilities.into_iter().collect();
        let last = queue.back().copied().unwrap_or(0.0);
        Self {
            queue,
            last,
            sample_rate: 16_000,
            chunk_size: 512,
        }
    }

    /// Construct a constant-probability mock (every chunk returns
    /// the same value).
    #[must_use]
    pub fn constant(probability: f32) -> Self {
        Self {
            queue: std::collections::VecDeque::new(),
            last: probability,
            sample_rate: 16_000,
            chunk_size: 512,
        }
    }
}

impl VadModel for MockVad {
    fn predict(&mut self, _samples: &[f32]) -> Result<f32, VoiceError> {
        let p = if let Some(next) = self.queue.pop_front() {
            self.last = next;
            next
        } else {
            self.last
        };
        Ok(p)
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn chunk_size(&self) -> usize {
        self.chunk_size
    }
}

// --- SileroVad (feature = "audio") ----------------------------------------

/// Real VAD via Silero V5. Behind `feature = "audio"`.
///
/// Wraps the `voice_activity_detector` crate. Hard-coded to 16kHz
/// with 512-sample chunks (the only configuration Silero V5
/// supports at 16kHz, per its docs). Other sample rates would
/// require a resampling layer in the audio capture path.
#[cfg(feature = "audio")]
pub struct SileroVad {
    inner: voice_activity_detector::VoiceActivityDetector,
}

#[cfg(feature = "audio")]
impl SileroVad {
    /// Construct a Silero V5 VAD at 16kHz with 512-sample chunks.
    pub fn new() -> Result<Self, VoiceError> {
        let inner = voice_activity_detector::VoiceActivityDetector::builder()
            .sample_rate(16_000_i64)
            .chunk_size(512_usize)
            .build()
            .map_err(|e| VoiceError::VadSetup(format!("{e}")))?;
        Ok(Self { inner })
    }
}

#[cfg(feature = "audio")]
impl VadModel for SileroVad {
    fn predict(&mut self, samples: &[f32]) -> Result<f32, VoiceError> {
        // The crate truncates/pads to chunk_size internally per its
        // docs. We pass the buffer through.
        Ok(self.inner.predict(samples.iter().copied()))
    }

    fn sample_rate(&self) -> u32 {
        16_000
    }

    fn chunk_size(&self) -> usize {
        512
    }
}

// --- VadGate --------------------------------------------------------------

/// Events emitted by the [`VadGate`] state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VadEvent {
    /// Speech onset — the gate observed `speech_threshold` exceeded
    /// for `speech_chunks` consecutive chunks.
    SpeechStart,
    /// Speech offset — the gate observed `silence_threshold` not
    /// exceeded for `silence_chunks` consecutive chunks after a
    /// `SpeechStart`.
    SpeechEnd,
}

/// Configuration for [`VadGate`]. Defaults match the
/// `parakeet-rs`-friendly profile from the v0.2 research.
#[derive(Debug, Clone, Copy)]
pub struct VadGateConfig {
    /// Probability threshold for declaring speech. Default 0.5.
    pub speech_threshold: f32,
    /// Probability threshold below which a chunk counts as silence.
    /// Lower than `speech_threshold` to debounce. Default 0.35.
    pub silence_threshold: f32,
    /// Consecutive speech chunks required to declare onset. Default 3
    /// (≈ 100ms at 32ms chunks).
    pub speech_chunks: u32,
    /// Consecutive silence chunks required to declare offset. Default
    /// 15 (≈ 480ms).
    pub silence_chunks: u32,
}

impl Default for VadGateConfig {
    fn default() -> Self {
        Self {
            speech_threshold: 0.5,
            silence_threshold: 0.35,
            speech_chunks: 3,
            silence_chunks: 15,
        }
    }
}

/// State machine that turns a stream of VAD probabilities into
/// utterance-boundary events.
///
/// The gate is what makes the VAD useful — raw per-chunk
/// probabilities flicker too much to drive an STT directly. The gate
/// applies hysteresis: speech is "on" only after `speech_chunks`
/// consecutive over-threshold chunks, "off" only after
/// `silence_chunks` consecutive under-threshold chunks.
#[derive(Debug, Clone, Copy)]
pub struct VadGate {
    config: VadGateConfig,
    state: GateState,
    speech_run: u32,
    silence_run: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GateState {
    /// Pre-speech idle. Counting toward `SpeechStart`.
    Idle,
    /// In-speech. Counting silence runs toward `SpeechEnd`.
    Speaking,
}

impl VadGate {
    /// Construct with default thresholds.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(VadGateConfig::default())
    }

    /// Construct with a specific config.
    #[must_use]
    pub fn with_config(config: VadGateConfig) -> Self {
        Self {
            config,
            state: GateState::Idle,
            speech_run: 0,
            silence_run: 0,
        }
    }

    /// Feed one VAD probability. Returns `Some(VadEvent)` if the
    /// state transitioned, `None` otherwise.
    pub fn observe(&mut self, probability: f32) -> Option<VadEvent> {
        match self.state {
            GateState::Idle => {
                if probability >= self.config.speech_threshold {
                    self.speech_run += 1;
                    self.silence_run = 0;
                    if self.speech_run >= self.config.speech_chunks {
                        self.state = GateState::Speaking;
                        self.speech_run = 0;
                        return Some(VadEvent::SpeechStart);
                    }
                } else {
                    self.speech_run = 0;
                }
                None
            }
            GateState::Speaking => {
                if probability < self.config.silence_threshold {
                    self.silence_run += 1;
                    if self.silence_run >= self.config.silence_chunks {
                        self.state = GateState::Idle;
                        self.silence_run = 0;
                        return Some(VadEvent::SpeechEnd);
                    }
                } else {
                    // Above silence threshold; reset silence run. Don't
                    // require above-speech-threshold to stay engaged —
                    // hysteresis means we hold on through the
                    // [silence_threshold, speech_threshold) band.
                    self.silence_run = 0;
                }
                None
            }
        }
    }

    /// Reset to idle. Used between utterances or on session restart.
    pub fn reset(&mut self) {
        self.state = GateState::Idle;
        self.speech_run = 0;
        self.silence_run = 0;
    }

    /// Whether the gate is currently in the Speaking state.
    #[must_use]
    pub fn is_speaking(&self) -> bool {
        self.state == GateState::Speaking
    }
}

impl Default for VadGate {
    fn default() -> Self {
        Self::new()
    }
}
