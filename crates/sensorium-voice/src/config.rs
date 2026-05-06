//! Configuration types for the voice substrate.

use std::path::PathBuf;
use std::time::Duration;

/// Which STT backend to use.
///
/// v0.2 ships `Mock` (canned responses for tests) and `Parakeet`
/// (real ONNX inference). v0.3 will add `Whisper` for multilingual
/// fallback.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Backend {
    /// Scripted-response backend for tests. Holds a list of canned
    /// transcripts and emits one per `transcribe` call.
    Mock {
        /// Canned responses; emitted in order, last one repeats.
        responses: Vec<String>,
    },
    /// NVIDIA Parakeet TDT (EOU streaming variant). Loads ONNX
    /// weights from the configured directory (or downloads them on
    /// first run via `hf-hub`).
    Parakeet {
        /// Where to look for / cache the EOU model weights.
        /// `None` defaults to `~/.cache/sensorium-voice/parakeet-eou/`.
        weights_dir: Option<PathBuf>,
    },
}

impl Backend {
    /// Quick constructor for a `Mock` backend with one canned response.
    #[must_use]
    pub fn mock(canned: impl Into<String>) -> Self {
        Self::Mock {
            responses: vec![canned.into()],
        }
    }
}

/// Per-session configuration for [`crate::VoiceSession`].
///
/// Construct via field literals or `VoiceConfig::default()` /
/// `VoiceConfig::mock(...)`. Not marked `#[non_exhaustive]` because
/// callers need to construct it through struct expressions; new
/// fields are added at the end and given sensible defaults via
/// [`VoiceConfig::default`].
#[derive(Debug, Clone)]
pub struct VoiceConfig {
    /// Which STT backend to dispatch to.
    pub backend: Backend,
    /// Target sample rate (always 16kHz for Parakeet / Silero VAD).
    /// Resampling happens transparently if the input device differs.
    pub sample_rate: u32,
    /// Maximum recording window — backstop in case VAD never detects
    /// silence. Default: 30 seconds.
    pub max_recording: Duration,
    /// VAD probability threshold for "speech started" (0.0..=1.0).
    /// Default: 0.5. Higher = stricter; lower = more permissive.
    pub vad_speech_threshold: f32,
    /// VAD probability threshold for "speech ended". Lower than the
    /// onset threshold to debounce. Default: 0.35.
    pub vad_silence_threshold: f32,
    /// How many consecutive silence chunks (32ms each at 512-sample
    /// chunks @ 16kHz) before we declare end-of-utterance. Default:
    /// 15 ≈ 480ms.
    pub vad_silence_chunks: u32,
}

impl VoiceConfig {
    /// Build a config with the [`Backend::Mock`] variant for tests.
    #[must_use]
    pub fn mock(canned: impl Into<String>) -> Self {
        Self {
            backend: Backend::mock(canned),
            sample_rate: 16_000,
            max_recording: Duration::from_secs(30),
            vad_speech_threshold: 0.5,
            vad_silence_threshold: 0.35,
            vad_silence_chunks: 15,
        }
    }

    /// Build a config for the real Parakeet EOU backend, using the
    /// default weights directory (`~/.cache/sensorium-voice/...`).
    #[must_use]
    pub fn parakeet_default() -> Self {
        Self {
            backend: Backend::Parakeet { weights_dir: None },
            sample_rate: 16_000,
            max_recording: Duration::from_secs(30),
            vad_speech_threshold: 0.5,
            vad_silence_threshold: 0.35,
            vad_silence_chunks: 15,
        }
    }
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self::parakeet_default()
    }
}
