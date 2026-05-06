//! # sensorium-voice
//!
//! Voice-input substrate for the Sensorium / MIL stack. Streams
//! microphone audio through an energy-RMS voice activity detector
//! and NVIDIA Parakeet TDT (EOU streaming variant) on-device,
//! emitting `PrimitiveToken { kind: Predication }` tokens with full
//! provenance.
//!
//! Step #17 of `MIL-PROJECT.md` §11.2. The final piece of MIL Tier 3.
//!
//! ## Architecture
//!
//! ```text
//! microphone (cpal, CoreAudio on macOS)
//!    │
//!    ▼  16kHz f32 mono
//! ringbuf (lock-free SPSC)
//!    │
//!    ▼
//! VAD gate (EnergyVad — RMS energy, zero deps)
//!    │
//!    ▼  speech-only audio
//! SpeechToText backend (Parakeet EOU / Mock / future Whisper)
//!    │
//!    ▼  streaming partials at 160ms cadence
//! PrimitiveToken { kind: Predication, payload: text, ... }
//!    │
//!    ▼  mpsc::channel
//! Consumer (pneuma-demo, future pneuma-binder)
//! ```
//!
//! ## Public API
//!
//! - [`SpeechToText`] — backend trait. Implementations: [`MockStt`],
//!   `ParakeetStt` (cfg-gated to `feature = "parakeet"` so
//!   ONNX-runtime-deps stay optional).
//! - [`VoiceSession`] — composition of audio capture + VAD + STT.
//!   `start()` spawns the pipeline; `tokens()` returns a receiver
//!   for `PrimitiveToken`s.
//! - [`VoiceConfig`] — per-session knobs (backend, sample rate, VAD
//!   thresholds).
//!
//! ## What this crate is NOT
//!
//! - **Not a parser.** Producing structured directives from the
//!   transcribed text is `pneuma-resolver`'s and the parser's job.
//!   This crate emits raw `Predication` tokens.
//! - **Not async.** v0.2 runs the audio + inference threads as
//!   `std::thread`. A future `tokio`-flavored variant will live
//!   alongside.
//! - **Not multi-channel.** Mono mic only. Stereo / multi-mic is
//!   v0.3.

#![doc = include_str!("../README.md")]

mod backend;
mod config;
mod error;
mod session;
mod token;
mod vad;

#[cfg(feature = "audio")]
mod audio;

pub use backend::{MockStt, SpeechToText, TranscriptDelta};
pub use config::{Backend, VoiceConfig};
pub use error::VoiceError;
pub use session::VoiceSession;
pub use token::predication_token;
pub use vad::{EnergyVad, MockVad, VadEvent, VadGate, VadGateConfig, VadModel};

#[cfg(feature = "audio")]
pub use audio::{AudioCapture, AudioCaptureConfig};

#[cfg(feature = "parakeet")]
pub use parakeet::ParakeetStt;

#[cfg(feature = "parakeet")]
mod parakeet;
