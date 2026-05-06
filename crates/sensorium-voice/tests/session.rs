//! Integration tests for [`VoiceSession`].
//!
//! Default-feature tests cover the Mock backend path end-to-end.
//! Real Parakeet inference lives in `parakeet_inference.rs`, gated
//! behind `feature = "parakeet"` and `#[ignore]`'d for the live
//! ONNX path.
//!
//! Properties under test:
//!
//! 1. **Mock backend constructs cleanly** — `VoiceConfig::mock(...)`
//!    yields a usable session.
//! 2. **Parakeet without feature flag errors gracefully** —
//!    `Backend::Parakeet` returns `BackendSetup` with a clear
//!    message when the `parakeet` feature is OFF (the default).
//!    Skipped when the feature is ON because the backend actually
//!    constructs in that build.
//! 3. **`feed` is a no-op for Mock** — the backend ignores audio
//!    chunks (returns `None` deltas).
//! 4. **`flush` emits a Final token** — the canned response surfaces
//!    on the channel as `PrimitiveToken::Predication`.
//! 5. **Multiple flushes drain the canned queue** — back-to-back
//!    flushes emit sequential canned responses.
//! 6. **Tokens carry voice provenance** — emitted tokens have
//!    `PrimitiveKind::Predication`, `PrivacyTier::Private`, and
//!    `Calibration::Uncalibrated`.
//! 7. **`tokens()` is one-shot** — second call returns `None` since
//!    the receiver was already taken.

use sensorium_core::{PrimitiveToken, PrivacyTier};
#[cfg(not(feature = "parakeet"))]
use sensorium_voice::VoiceError;
use sensorium_voice::{Backend, MockStt, SpeechToText, VoiceConfig, VoiceSession};

// --- Property 1: Constructor -----------------------------------------------

#[test]
fn mock_session_constructs_cleanly() {
    let session = VoiceSession::new(VoiceConfig::mock("hello"));
    assert!(session.is_ok());
}

// --- Property 2: Parakeet stub error ---------------------------------------
//
// Only meaningful when the `parakeet` feature is OFF. With the
// feature ON, `Backend::Parakeet` actually constructs a real
// `ParakeetStt` (and may even download weights), which is the
// exact opposite of what this test asserts.

#[cfg(not(feature = "parakeet"))]
#[test]
fn parakeet_backend_without_feature_flag_errors_with_helpful_message() {
    let cfg = VoiceConfig {
        backend: Backend::Parakeet { weights_dir: None },
        ..VoiceConfig::default()
    };
    // VoiceSession is not Debug (owns Box<dyn SpeechToText>), so
    // `unwrap_err` is unavailable; let-else extracts the err.
    let Err(err) = VoiceSession::new(cfg) else {
        panic!("Parakeet without feature must error")
    };
    match err {
        VoiceError::BackendSetup(msg) => {
            assert!(
                msg.contains("Parakeet"),
                "error message must mention Parakeet; got: {msg}"
            );
            assert!(
                msg.contains("v0.2") || msg.contains("Mock") || msg.contains("feature"),
                "error must guide caller to the v0.2 fallback; got: {msg}"
            );
        }
        other => panic!("expected BackendSetup, got {other:?}"),
    }
}

// --- Property 3: feed is silent for Mock -----------------------------------

#[test]
fn feed_chunk_is_silent_for_mock_backend() {
    let mut session = VoiceSession::new(VoiceConfig::mock("ignored")).unwrap();
    let rx = session.tokens().unwrap();

    // Feed a tiny audio chunk (256 samples of silence).
    let chunk = vec![0.0_f32; 256];
    session.feed(&chunk).unwrap();

    // Mock emits only on flush — channel should be empty.
    assert!(rx.try_recv().is_err());
}

// --- Property 4: flush emits Final token ----------------------------------

#[test]
fn flush_emits_predication_token() {
    let mut session = VoiceSession::new(VoiceConfig::mock("rename to bar.txt")).unwrap();
    let rx = session.tokens().unwrap();
    session.flush().unwrap();

    let token = rx.try_recv().expect("flush must emit a token");
    match token {
        PrimitiveToken::Predication(t) => {
            assert_eq!(t.value, "rename to bar.txt");
        }
        other => panic!("expected Predication, got {other:?}"),
    }
}

// --- Property 5: Multiple flushes drain the queue --------------------------

#[test]
fn back_to_back_flushes_emit_sequential_canned_responses() {
    let mut session = VoiceSession::new(VoiceConfig {
        backend: Backend::Mock {
            responses: vec!["first utterance".into(), "second utterance".into()],
        },
        ..VoiceConfig::default()
    })
    .unwrap();
    let rx = session.tokens().unwrap();

    session.flush().unwrap();
    session.flush().unwrap();

    let texts: Vec<String> = (0..2)
        .map(|_| {
            let tok = rx.try_recv().expect("each flush emits");
            match tok {
                PrimitiveToken::Predication(t) => t.value,
                _ => panic!("non-predication token"),
            }
        })
        .collect();
    assert_eq!(texts, vec!["first utterance", "second utterance"]);
}

#[test]
fn flush_after_queue_drains_repeats_last_response() {
    let mut session = VoiceSession::new(VoiceConfig::mock("only one")).unwrap();
    let rx = session.tokens().unwrap();
    session.flush().unwrap();
    session.flush().unwrap();
    session.flush().unwrap();

    let mut count = 0;
    while let Ok(token) = rx.try_recv() {
        match token {
            PrimitiveToken::Predication(t) => assert_eq!(t.value, "only one"),
            _ => panic!("non-predication token"),
        }
        count += 1;
    }
    assert_eq!(count, 3, "all three flushes must emit");
}

// --- Property 6: Token provenance is correct -------------------------------

#[test]
fn emitted_token_carries_voice_provenance() {
    let mut session = VoiceSession::new(VoiceConfig::mock("hi")).unwrap();
    let rx = session.tokens().unwrap();
    session.flush().unwrap();

    let token = rx.try_recv().unwrap();
    let provenance = token.provenance();
    assert_eq!(
        provenance.primitive,
        sensorium_core::PrimitiveKind::Predication
    );
    assert_eq!(provenance.privacy, PrivacyTier::Private);
    // Calibration is uncalibrated for v0.2 — we apply the 20%
    // confidence penalty downstream.
    assert!(!provenance.is_calibrated());
}

// --- Property 7: tokens() is one-shot --------------------------------------

#[test]
fn tokens_receiver_can_only_be_taken_once() {
    let mut session = VoiceSession::new(VoiceConfig::mock("hi")).unwrap();
    assert!(session.tokens().is_some());
    assert!(session.tokens().is_none(), "second call must return None");
}

// --- MockStt direct usage --------------------------------------------------
//
// Doesn't require a session — verifies the backend can be used in
// isolation by callers who want to drive their own audio pipeline.

#[test]
fn mock_stt_used_directly_emits_canned_on_flush() {
    let mut backend = MockStt::new(["hello world"]);
    assert!(backend.transcribe_chunk(&[0.0; 256]).unwrap().is_none());
    let delta = backend.flush().unwrap().unwrap();
    assert!(delta.is_final());
    assert_eq!(delta.text(), "hello world");
    assert_eq!(backend.label(), "mock");
}
