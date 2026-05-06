//! Unit tests for the [`VadGate`] hysteresis state machine.
//!
//! The gate logic is what turns flickery per-chunk VAD probabilities
//! into stable utterance-boundary events. We test it with `MockVad`
//! / direct probability injection so the test stays cross-platform
//! and audio-hardware-free.
//!
//! Properties under test:
//!
//! 1. **Idle by default** — fresh gate is not speaking.
//! 2. **Onset requires N consecutive over-threshold** — single
//!    spike does not trigger SpeechStart.
//! 3. **Offset requires M consecutive under-threshold** — brief
//!    silence does not trigger SpeechEnd.
//! 4. **Onset emits exactly once per utterance** — observing more
//!    speech after Speaking does not re-emit SpeechStart.
//! 5. **Offset emits exactly once per utterance** — observing more
//!    silence after Idle does not re-emit SpeechEnd.
//! 6. **Hysteresis band holds** — probabilities in
//!    `[silence_threshold, speech_threshold)` keep the gate in
//!    whichever state it was already in.
//! 7. **Reset clears state** — after `reset()`, the gate is Idle
//!    with zero counters.
//! 8. **Custom config** — adjustable thresholds + chunk counts work.

use sensorium_voice::{EnergyVad, MockVad, VadEvent, VadGate, VadGateConfig, VadModel};

// --- Property 1 -----------------------------------------------------------

#[test]
fn fresh_gate_is_idle() {
    let g = VadGate::new();
    assert!(!g.is_speaking());
}

// --- Property 2 -----------------------------------------------------------

#[test]
fn onset_requires_consecutive_over_threshold() {
    let mut g = VadGate::new();
    // Default: speech_threshold=0.5, speech_chunks=3.
    assert_eq!(g.observe(0.9), None);
    assert_eq!(g.observe(0.9), None);
    assert_eq!(g.observe(0.9), Some(VadEvent::SpeechStart));
    assert!(g.is_speaking());
}

#[test]
fn single_spike_does_not_trigger_onset() {
    let mut g = VadGate::new();
    assert_eq!(g.observe(0.9), None);
    assert_eq!(g.observe(0.1), None);
    assert_eq!(g.observe(0.9), None);
    assert!(!g.is_speaking());
}

// --- Property 3 -----------------------------------------------------------

#[test]
fn offset_requires_consecutive_under_threshold() {
    let mut g = VadGate::new();
    // Walk into Speaking.
    assert_eq!(g.observe(0.9), None);
    assert_eq!(g.observe(0.9), None);
    assert_eq!(g.observe(0.9), Some(VadEvent::SpeechStart));

    // Default silence_chunks=15, silence_threshold=0.35. Need 15
    // consecutive under-threshold chunks to declare offset.
    for _ in 0..14 {
        assert_eq!(g.observe(0.0), None);
    }
    assert_eq!(g.observe(0.0), Some(VadEvent::SpeechEnd));
    assert!(!g.is_speaking());
}

#[test]
fn brief_silence_does_not_trigger_offset() {
    let mut g = VadGate::new();
    for _ in 0..3 {
        g.observe(0.9);
    }
    assert!(g.is_speaking());
    // 5 silent chunks (less than silence_chunks=15)
    for _ in 0..5 {
        assert_eq!(g.observe(0.0), None);
    }
    assert!(g.is_speaking());
    // Speech resumes — silence run resets.
    for _ in 0..3 {
        g.observe(0.9);
    }
    // No second SpeechStart (we never left Speaking).
    assert!(g.is_speaking());
}

// --- Property 4 -----------------------------------------------------------

#[test]
fn onset_emits_exactly_once_per_utterance() {
    let mut g = VadGate::new();
    let mut starts = 0;
    for _ in 0..20 {
        if matches!(g.observe(0.9), Some(VadEvent::SpeechStart)) {
            starts += 1;
        }
    }
    assert_eq!(starts, 1);
}

// --- Property 5 -----------------------------------------------------------

#[test]
fn offset_emits_exactly_once_per_utterance() {
    let mut g = VadGate::new();
    for _ in 0..3 {
        g.observe(0.9);
    }
    let mut ends = 0;
    for _ in 0..50 {
        if matches!(g.observe(0.0), Some(VadEvent::SpeechEnd)) {
            ends += 1;
        }
    }
    assert_eq!(ends, 1);
}

// --- Property 6: Hysteresis band ------------------------------------------

#[test]
fn band_probability_holds_speaking_state() {
    let mut g = VadGate::new();
    // Walk into Speaking with strong signal.
    for _ in 0..3 {
        g.observe(0.9);
    }
    // Hover in the [0.35, 0.5) band — between the two thresholds.
    // Should NOT emit SpeechEnd.
    for _ in 0..30 {
        assert_eq!(g.observe(0.4), None);
    }
    assert!(g.is_speaking(), "band probability must hold Speaking");
}

#[test]
fn band_probability_holds_idle_state() {
    let mut g = VadGate::new();
    // Hover in the band without ever fully crossing speech threshold.
    for _ in 0..30 {
        assert_eq!(g.observe(0.4), None);
    }
    assert!(
        !g.is_speaking(),
        "band probability alone never triggers onset"
    );
}

// --- Property 7: Reset ----------------------------------------------------

#[test]
fn reset_clears_state_and_counters() {
    let mut g = VadGate::new();
    g.observe(0.9);
    g.observe(0.9);
    // Gate is mid-onset (1 more chunk would trigger). Reset.
    g.reset();
    assert!(!g.is_speaking());
    // Need a full new run of 3 chunks; 2 alone won't trigger.
    g.observe(0.9);
    g.observe(0.9);
    assert!(!g.is_speaking());
}

// --- Property 8: Custom config --------------------------------------------

#[test]
fn custom_config_with_single_chunk_thresholds() {
    let mut g = VadGate::with_config(VadGateConfig {
        speech_threshold: 0.6,
        silence_threshold: 0.3,
        speech_chunks: 1,
        silence_chunks: 1,
    });
    assert_eq!(g.observe(0.7), Some(VadEvent::SpeechStart));
    assert_eq!(g.observe(0.1), Some(VadEvent::SpeechEnd));
}

// --- MockVad smoke check --------------------------------------------------

#[test]
#[allow(clippy::float_cmp)]
fn mock_vad_returns_canned_probabilities() {
    // Exact float equality is the right check here — we're storing
    // f32 values verbatim in a queue and reading them back. No
    // arithmetic, no precision loss.
    let mut m = MockVad::new([0.1, 0.7, 0.9]);
    assert_eq!(m.predict(&[]).unwrap(), 0.1);
    assert_eq!(m.predict(&[]).unwrap(), 0.7);
    assert_eq!(m.predict(&[]).unwrap(), 0.9);
    // Drained → repeats last.
    assert_eq!(m.predict(&[]).unwrap(), 0.9);
}

#[test]
fn mock_vad_constant_returns_constant() {
    let mut m = MockVad::constant(0.42);
    for _ in 0..5 {
        assert!((m.predict(&[]).unwrap() - 0.42).abs() < f32::EPSILON);
    }
}

#[test]
fn mock_vad_default_chunk_size_is_silero_compatible() {
    let m = MockVad::constant(0.0);
    assert_eq!(m.sample_rate(), 16_000);
    assert_eq!(m.chunk_size(), 512);
}

// --- EnergyVad smoke checks -----------------------------------------------

#[test]
fn energy_vad_silence_returns_zero_probability() {
    let mut v = EnergyVad::new();
    let silence = vec![0.0_f32; 512];
    let p = v.predict(&silence).unwrap();
    assert!((p - 0.0).abs() < f32::EPSILON, "silence → 0, got {p}");
}

#[test]
fn energy_vad_loud_signal_returns_high_probability() {
    let mut v = EnergyVad::new();
    // Sine-like-ish: sustained 0.5 amplitude. RMS = 0.5 — well above
    // the default speech_floor (0.032).
    let loud: Vec<f32> = (0..512)
        .map(|i| if i % 2 == 0 { 0.5 } else { -0.5 })
        .collect();
    let p = v.predict(&loud).unwrap();
    assert!((p - 1.0).abs() < f32::EPSILON, "loud signal → 1, got {p}");
}

#[test]
fn energy_vad_band_returns_intermediate_probability() {
    // RMS halfway between thresholds (~0.018) → ~0.5 probability.
    let mut v = EnergyVad::new();
    let amp = 0.018_f32;
    let band: Vec<f32> = (0..512)
        .map(|i| if i % 2 == 0 { amp } else { -amp })
        .collect();
    let p = v.predict(&band).unwrap();
    assert!(p > 0.0 && p < 1.0, "band → (0, 1), got {p}");
}

#[test]
fn energy_vad_default_chunk_size_matches_silero() {
    let v = EnergyVad::new();
    assert_eq!(v.sample_rate(), 16_000);
    assert_eq!(v.chunk_size(), 512);
}

#[test]
fn energy_vad_custom_thresholds() {
    let mut v = EnergyVad::with_thresholds(0.01, 0.02);
    // RMS ≈ 0.005 — below silence_floor → 0.
    let quiet: Vec<f32> = (0..512)
        .map(|i| if i % 2 == 0 { 0.005 } else { -0.005 })
        .collect();
    assert!((v.predict(&quiet).unwrap() - 0.0).abs() < f32::EPSILON);
}

#[test]
fn energy_vad_drives_gate_end_to_end() {
    // Real plumbing: feed alternating silence + loud chunks through
    // the Vad → Gate pipeline and assert utterance-boundary events.
    let mut v = EnergyVad::new();
    let mut g = VadGate::with_config(VadGateConfig {
        speech_threshold: 0.6,
        silence_threshold: 0.3,
        speech_chunks: 1,
        silence_chunks: 1,
    });

    let silence = vec![0.0_f32; 512];
    let loud: Vec<f32> = (0..512)
        .map(|i| if i % 2 == 0 { 0.5 } else { -0.5 })
        .collect();

    // 2 silent chunks → no event.
    for _ in 0..2 {
        let p = v.predict(&silence).unwrap();
        assert_eq!(g.observe(p), None);
    }
    // 1 loud chunk → SpeechStart (tight gate config).
    let p_loud = v.predict(&loud).unwrap();
    assert_eq!(g.observe(p_loud), Some(VadEvent::SpeechStart));
    // 1 silent chunk → SpeechEnd.
    let p_silent = v.predict(&silence).unwrap();
    assert_eq!(g.observe(p_silent), Some(VadEvent::SpeechEnd));
}
