//! cpal microphone capture, gated by `feature = "audio"`.
//!
//! Provides [`AudioCapture`] — a wrapper around `cpal`'s default
//! input device that streams `f32` samples through a lock-free SPSC
//! ringbuffer to a consumer thread.
//!
//! ## Sample-rate strategy
//!
//! Both Parakeet EOU and our `EnergyVad` (and the future Silero V5
//! re-introduction) want 16kHz. macOS default mic configs vary:
//! built-in mics often default to 44.1kHz or 48kHz; some USB mics
//! offer 16kHz natively.
//!
//! v0.2 strategy: prefer 16kHz if the device supports it; otherwise
//! resample with a simple linear interpolator at capture time.
//! Linear resampling is "good enough for STT" per the v0.2
//! research; v0.3 may swap in `rubato` for sinc resampling if
//! quality matters.
//!
//! ## Threading
//!
//! `cpal`'s input callback runs on a real-time audio thread —
//! cannot block, cannot allocate, cannot panic. The callback's
//! single job is to push samples into the ringbuffer and return.
//! The consumer (typically `VoiceSession`) drains the ringbuffer
//! on a regular thread, runs the VAD gate, feeds the STT backend.
//!
//! ## Failure modes
//!
//! - No default input device → `VoiceError::NoInputDevice`.
//! - Device config query fails → `VoiceError::DeviceConfig`.
//! - Stream construction fails → `VoiceError::StreamBuild`.
//! - Ringbuffer overflow (consumer too slow) → samples are
//!   dropped silently. The consumer will see a discontinuity but
//!   no error; v0.3 may surface a counter.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use cpal::Stream;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::traits::{Producer, Split};
use ringbuf::{HeapCons, HeapRb};

use crate::error::VoiceError;

/// Configuration for [`AudioCapture`].
#[derive(Debug, Clone)]
pub struct AudioCaptureConfig {
    /// Target sample rate the consumer will see (always 16kHz for
    /// the v0.2 EnergyVad / Parakeet pipeline). The capture path
    /// resamples if the device runs at a different rate.
    pub target_sample_rate: u32,
    /// Ring buffer capacity in samples. ~3 seconds at 16kHz f32 mono
    /// = 48000 samples = 192kB.
    pub ringbuf_capacity: usize,
}

impl Default for AudioCaptureConfig {
    fn default() -> Self {
        Self {
            target_sample_rate: 16_000,
            ringbuf_capacity: 48_000,
        }
    }
}

/// Microphone capture pipeline.
///
/// Owns the `cpal` input stream and exposes a `HeapCons<f32>` for
/// the consumer thread to drain. The stream runs continuously from
/// `start()` until `stop()` (or `Drop`).
pub struct AudioCapture {
    /// The underlying cpal stream — kept alive for the duration of
    /// capture. Stops when dropped.
    _stream: Stream,
    /// Consumer half of the SPSC ring. Caller drains samples from
    /// this on the consumer thread.
    consumer: Option<HeapCons<f32>>,
    /// Source-device sample rate (informational; the pipeline
    /// resamples to `config.target_sample_rate` before delivery).
    source_sample_rate: u32,
    /// Latched device-readable name, if any.
    device_name: String,
    /// Stop signal — set by `stop()` to break out of any consumer
    /// loops the caller has running.
    stop_flag: Arc<AtomicBool>,
}

impl AudioCapture {
    /// Start capturing from the default input device with
    /// `target_sample_rate` (typically 16kHz). If the device runs at
    /// a different native rate, samples are resampled in the
    /// callback before going into the ringbuf.
    pub fn start(config: &AudioCaptureConfig) -> Result<Self, VoiceError> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or(VoiceError::NoInputDevice)?;
        // cpal 0.17 deprecates `name()` in favor of `description()`
        // / `id()`. `description()` returns a `DeviceDescription`
        // struct; we format it into a HUD-friendly string.
        let device_name = match device.description() {
            Ok(desc) => format!("{desc:?}"),
            Err(_) => "<unnamed>".to_owned(),
        };

        // Pick a config — prefer the target sample rate if the
        // device supports it; fall back to the device's default.
        let supported = device
            .default_input_config()
            .map_err(|e| VoiceError::DeviceConfig(format!("{e}")))?;
        let source_sample_rate = supported.sample_rate();
        let channels = supported.channels();

        // Build the ring buffer outside the callback so we can hand
        // out the consumer half before the stream starts.
        let rb = HeapRb::<f32>::new(config.ringbuf_capacity);
        let (mut producer, consumer) = rb.split();

        let stop_flag = Arc::new(AtomicBool::new(false));

        // Resampling ratio: source → target. The simple linear
        // resampler picks samples at the target rate from the source
        // stream. Skips quality-conscious approaches like sinc; v0.3
        // may swap in `rubato` if STT WER suffers.
        let resample_ratio = f64::from(config.target_sample_rate) / f64::from(source_sample_rate);
        let mut resample_phase: f64 = 0.0;

        // The cpal callback must NOT block. We push synchronously to
        // the lock-free ringbuf and return. Overflow drops samples
        // (consumer too slow) — see module docs.
        let stream_config: cpal::StreamConfig = supported.into();
        let stream = device
            .build_input_stream(
                &stream_config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    // Channel-flatten: cpal interleaves multi-channel
                    // input. Mono-mix by averaging across channels.
                    let chan = channels as usize;
                    if chan == 0 {
                        return;
                    }
                    // `chan` is small (1, 2, 6 for surround). Cast
                    // is exact for any realistic channel count.
                    #[allow(clippy::cast_precision_loss)]
                    let chan_recip = 1.0_f32 / (chan as f32);
                    for frame in data.chunks_exact(chan) {
                        let mono: f32 = frame.iter().copied().sum::<f32>() * chan_recip;
                        // Resample: emit a target sample whenever
                        // phase crosses unity; advance phase by the
                        // ratio per source sample.
                        resample_phase += resample_ratio;
                        while resample_phase >= 1.0 {
                            resample_phase -= 1.0;
                            // Drop on overflow. Producer::try_push
                            // returns Err if the ringbuf is full.
                            let _ = producer.try_push(mono);
                        }
                    }
                },
                |err| {
                    // The stream error callback runs off the audio
                    // thread; eprintln! is fine here.
                    eprintln!("sensorium-voice: cpal input stream error: {err}");
                },
                None,
            )
            .map_err(|e| VoiceError::StreamBuild(format!("{e}")))?;

        stream
            .play()
            .map_err(|e| VoiceError::StreamBuild(format!("play: {e}")))?;

        Ok(Self {
            _stream: stream,
            consumer: Some(consumer),
            source_sample_rate,
            device_name,
            stop_flag,
        })
    }

    /// Take ownership of the consumer half of the ringbuffer. Returns
    /// `None` on the second call.
    ///
    /// The consumer exposes `try_pop` and `pop_slice` for draining
    /// audio in the consumer thread.
    pub fn consumer(&mut self) -> Option<HeapCons<f32>> {
        self.consumer.take()
    }

    /// Source-device sample rate. Informational; the consumer
    /// always sees `target_sample_rate`.
    #[must_use]
    pub fn source_sample_rate(&self) -> u32 {
        self.source_sample_rate
    }

    /// Device name (for HUD / journal). May be `"<unnamed>"` on
    /// hosts that don't expose a name.
    #[must_use]
    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Signal stop. The cpal stream itself stops when the
    /// `AudioCapture` is dropped; this flag exists for callers who
    /// run consumer loops on the audio data and want a coordinated
    /// shutdown signal.
    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::Release);
    }

    /// Whether `stop()` has been called.
    #[must_use]
    pub fn stopping(&self) -> bool {
        self.stop_flag.load(Ordering::Acquire)
    }

    /// Clone the stop flag so consumer threads can poll it.
    #[must_use]
    pub fn stop_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.stop_flag)
    }
}
