//! Real Parakeet TDT EOU streaming inference tests.
//!
//! Feature-gated to `parakeet`; the live tests are `#[ignore]`'d
//! because they:
//!
//! 1. Trigger a ~150MB weight download from Hugging Face on first
//!    run (cached afterward).
//! 2. Run actual ONNX inference, which is slow on CI.
//!
//! Run manually:
//!
//! ```bash
//! cargo test -p sensorium-voice --features parakeet --test parakeet_inference -- --ignored
//! ```
//!
//! Properties under test:
//!
//! 1. (`#[ignore]`'d) `ParakeetStt::new(None)` succeeds — bootstrap
//!    weights, construct the model.
//! 2. (`#[ignore]`'d) Feeding a synthetic-silence audio buffer
//!    through `transcribe_chunk` + `flush` returns a `Final` delta
//!    (likely empty for pure silence).
//! 3. (`#[ignore]`'d) `VoiceSession::new(VoiceConfig::parakeet_default())`
//!    constructs without erroring (with weights cached).

#![cfg(feature = "parakeet")]

use sensorium_voice::{
    Backend, ParakeetStt, SpeechToText, TranscriptDelta, VoiceConfig, VoiceSession,
};

#[test]
#[ignore = "downloads ~150MB on first run; loads ONNX runtime"]
fn parakeet_stt_constructs_with_default_cache() {
    let stt = ParakeetStt::new(None).expect("bootstrap + construct");
    assert_eq!(stt.label(), "parakeet-eou-120m");
}

#[test]
#[ignore = "real ONNX inference"]
fn parakeet_stt_silence_yields_empty_final() {
    let mut stt = ParakeetStt::new(None).expect("setup");
    // ~1.6 seconds of silence in 2560-sample (160ms) chunks.
    let silence = vec![0.0_f32; 16_000];
    stt.transcribe_chunk(&silence).expect("transcribe_chunk");
    let delta = stt.flush().expect("flush").expect("Some delta");
    match delta {
        TranscriptDelta::Final { text } => {
            // Pure synthetic silence may yield empty text or
            // unprintable filler; we just assert the call succeeded
            // and produced *some* Final.
            eprintln!("silence final: {text:?}");
        }
        other => panic!("expected Final, got {other:?}"),
    }
}

#[test]
#[ignore = "real ONNX inference + downloads"]
fn voice_session_with_parakeet_backend_constructs() {
    let session = VoiceSession::new(VoiceConfig {
        backend: Backend::Parakeet { weights_dir: None },
        ..VoiceConfig::default()
    });
    match session {
        Ok(_) => {} // success path
        Err(e) => panic!("VoiceSession parakeet construction failed: {e}"),
    }
}
