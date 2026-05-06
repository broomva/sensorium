//! End-to-end test of [`VoiceSession::run_vad_driven`].
//!
//! Wires `MockVad` (programmable probabilities) + `VadGate`
//! (hysteresis state machine) + `VoiceSession` with `MockStt`
//! backend. Drives synthetic audio (`f32` zeros) and a scripted
//! probability sequence; asserts that the right tokens emerge on
//! the channel at the right utterance boundaries.
//!
//! This is the "audio plumbing test" that proves the pipeline
//! works without real microphone hardware. The macOS-gated
//! interactive test in `audio_pipeline.rs` covers the cpal +
//! Silero V5 + microphone end of things.

use sensorium_core::PrimitiveToken;
use sensorium_voice::{Backend, MockVad, VadGate, VadGateConfig, VoiceConfig, VoiceSession};

/// Build a VAD gate with single-chunk thresholds for tight test
/// control. Skips the natural-speech hysteresis windows.
fn tight_gate() -> VadGate {
    VadGate::with_config(VadGateConfig {
        speech_threshold: 0.6,
        silence_threshold: 0.3,
        speech_chunks: 1,
        silence_chunks: 1,
    })
}

#[test]
fn one_utterance_through_full_pipeline() {
    let mut session = VoiceSession::new(VoiceConfig {
        backend: Backend::Mock {
            responses: vec!["hello world".to_owned()],
        },
        ..VoiceConfig::default()
    })
    .expect("session");
    let rx = session.tokens().expect("tokens");

    // Probabilities: low (no event) → high (SpeechStart) → high
    // (during) → low (SpeechEnd). Use a tight gate so 1 chunk
    // either side triggers.
    let mut vad = MockVad::new([0.0, 0.9, 0.9, 0.0]);
    let mut gate = tight_gate();
    // 4 chunks × 512 samples = 2048 zeros.
    let samples = vec![0.0_f32; 4 * 512];

    let utterances = session
        .run_vad_driven(samples, &mut vad, &mut gate)
        .expect("driver");
    assert_eq!(utterances, 1, "exactly one SpeechEnd should fire");

    // The Mock backend emits its canned response on flush; flush
    // happens inside run_vad_driven on SpeechEnd.
    let token = rx.try_recv().expect("token emitted");
    match token {
        PrimitiveToken::Predication(t) => {
            assert_eq!(t.value, "hello world");
        }
        other => panic!("expected Predication, got {other:?}"),
    }
}

#[test]
fn two_utterances_emit_sequential_tokens() {
    let mut session = VoiceSession::new(VoiceConfig {
        backend: Backend::Mock {
            responses: vec!["first".to_owned(), "second".to_owned()],
        },
        ..VoiceConfig::default()
    })
    .expect("session");
    let rx = session.tokens().expect("tokens");

    // Two utterances in one stream.
    let mut vad = MockVad::new([
        // Utt 1: speech then silence
        0.0, 0.9, 0.9, 0.0, //
        // Pause
        0.0, 0.0, //
        // Utt 2: speech then silence
        0.9, 0.9, 0.0,
    ]);
    let mut gate = tight_gate();
    let samples = vec![0.0_f32; 9 * 512];

    let utterances = session
        .run_vad_driven(samples, &mut vad, &mut gate)
        .expect("driver");
    assert_eq!(utterances, 2);

    let texts: Vec<String> = (0..2)
        .map(|_| match rx.try_recv().expect("token") {
            PrimitiveToken::Predication(t) => t.value,
            _ => panic!("non-predication"),
        })
        .collect();
    assert_eq!(texts, vec!["first", "second"]);
}

#[test]
fn pure_silence_emits_no_tokens() {
    let mut session = VoiceSession::new(VoiceConfig::mock("never")).expect("session");
    let rx = session.tokens().expect("tokens");
    let mut vad = MockVad::constant(0.05);
    let mut gate = tight_gate();
    let samples = vec![0.0_f32; 50 * 512];

    let utterances = session
        .run_vad_driven(samples, &mut vad, &mut gate)
        .expect("driver");
    assert_eq!(utterances, 0);
    assert!(rx.try_recv().is_err(), "no tokens should emit");
}

#[test]
fn buffer_below_chunk_size_does_not_invoke_vad() {
    // 511 samples — one short of a Silero V5 chunk. Nothing should
    // happen: VAD never gets called, gate stays Idle, no tokens.
    let mut session = VoiceSession::new(VoiceConfig::mock("unused")).expect("session");
    let rx = session.tokens().expect("tokens");
    let mut vad = MockVad::constant(0.99);
    let mut gate = tight_gate();
    let samples = vec![0.0_f32; 511];

    let utterances = session
        .run_vad_driven(samples, &mut vad, &mut gate)
        .expect("driver");
    assert_eq!(utterances, 0);
    assert!(rx.try_recv().is_err());
}

#[test]
fn dangling_speech_without_silence_does_not_emit() {
    // Speech started but iterator drained mid-utterance — no
    // SpeechEnd, no flush, no token. Caller would call
    // session.flush() manually if they want the tail.
    let mut session = VoiceSession::new(VoiceConfig::mock("orphan")).expect("session");
    let rx = session.tokens().expect("tokens");
    let mut vad = MockVad::constant(0.9);
    let mut gate = tight_gate();
    let samples = vec![0.0_f32; 5 * 512];

    let utterances = session
        .run_vad_driven(samples, &mut vad, &mut gate)
        .expect("driver");
    assert_eq!(utterances, 0, "no SpeechEnd observed");
    assert!(rx.try_recv().is_err(), "no auto-flush mid-stream");

    // Manual flush surfaces the tail.
    session.flush().expect("flush");
    let token = rx.try_recv().expect("token after manual flush");
    match token {
        PrimitiveToken::Predication(t) => assert_eq!(t.value, "orphan"),
        _ => panic!("non-predication"),
    }
}
