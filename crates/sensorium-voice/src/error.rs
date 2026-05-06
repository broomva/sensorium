//! Errors raised by the voice substrate.

use thiserror::Error;

/// Errors raised by [`crate::VoiceSession`] and its backends.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum VoiceError {
    /// The system has no input audio device — laptop with mic
    /// disabled, headless CI, etc.
    #[error("no input audio device available")]
    NoInputDevice,

    /// Failed to query the input device's default config.
    #[error("audio device config query failed: {0}")]
    DeviceConfig(String),

    /// Failed to build the input stream (sample format mismatch,
    /// device disappeared, etc.).
    #[error("audio stream build failed: {0}")]
    StreamBuild(String),

    /// Failed to spawn the inference thread.
    #[error("inference thread spawn failed: {0}")]
    ThreadSpawn(std::io::Error),

    /// Failed to load Silero VAD weights.
    #[error("VAD setup failed: {0}")]
    VadSetup(String),

    /// Backend setup failed (e.g., Parakeet model couldn't load).
    #[error("STT backend setup failed: {0}")]
    BackendSetup(String),

    /// Backend inference call failed.
    #[error("STT inference failed: {0}")]
    Inference(String),

    /// Failed to download model weights from Hugging Face.
    #[error("weight download failed: {0}")]
    WeightDownload(String),
}
