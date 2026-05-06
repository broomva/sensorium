//! Audio pipeline tests, gated to `feature = "audio"`.
//!
//! Most of this file is gated behind `#[cfg(feature = "audio")]`
//! so default-feature CI builds skip it. The interactive live-mic
//! test inside is also `#[ignore]`'d so even with the feature
//! enabled it doesn't run unless invoked manually:
//!
//! ```bash
//! cargo test -p sensorium-voice --features audio --test audio_pipeline -- --ignored
//! ```
//!
//! Properties under test (with `--features audio`):
//!
//! 1. (`#[ignore]`'d) `AudioCapture::start` opens the default mic,
//!    streams ~1s, and the consumer drains real samples.
//!
//! `EnergyVad` and `VadGate` tests live in `vad_gate.rs` (always built).

#![cfg(feature = "audio")]

use std::time::Duration;

use ringbuf::traits::Consumer;
use sensorium_voice::{AudioCapture, AudioCaptureConfig};

/// Live-mic test. Requires a working input device and macOS
/// permission to access the microphone for the test runner. Skipped
/// by default — opens the user's mic, which CI / headless runs
/// can't reasonably do.
///
/// Run manually:
/// ```bash
/// cargo test -p sensorium-voice --features audio --test audio_pipeline -- --ignored
/// ```
#[test]
#[ignore = "opens the default microphone — run manually"]
fn audio_capture_live_mic_drains_real_samples() {
    let mut capture = AudioCapture::start(&AudioCaptureConfig::default())
        .expect("AudioCapture::start must succeed when a mic is available");

    let mut consumer = capture
        .consumer()
        .expect("first consumer() must yield Some");
    assert!(
        capture.consumer().is_none(),
        "second consumer() returns None"
    );

    // Let the stream warm up + buffer ~1s of audio.
    std::thread::sleep(Duration::from_millis(1100));

    let mut drained = Vec::<f32>::with_capacity(20_000);
    let mut buf = [0.0_f32; 1024];
    loop {
        let n = consumer.pop_slice(&mut buf);
        if n == 0 {
            break;
        }
        drained.extend_from_slice(&buf[..n]);
    }

    eprintln!(
        "device: {} | source_rate: {}Hz | drained: {} samples",
        capture.device_name(),
        capture.source_sample_rate(),
        drained.len()
    );

    // We expect at least *some* samples to have flowed in 1.1s. The
    // exact count varies with device buffer sizes; just assert
    // non-trivial volume.
    assert!(
        drained.len() > 1_000,
        "expected >1000 samples in 1.1s, got {}",
        drained.len()
    );

    // Samples should be in the f32 normalized range. A miked-up
    // input rarely floods the full range, but values must be finite
    // and within [-1, 1]-ish.
    for s in drained.iter().take(1024) {
        assert!(s.is_finite(), "sample {s} not finite");
        assert!(s.abs() < 2.0, "sample {s} suspiciously out of range");
    }

    capture.stop();
    assert!(capture.stopping());
}
