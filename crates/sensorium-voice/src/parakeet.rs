//! Real Parakeet TDT EOU streaming inference. `feature = "parakeet"`.
//!
//! Wraps `parakeet_rs::ParakeetEOU` — the EOU (end-of-utterance)
//! variant of NVIDIA Parakeet, optimized for streaming partials at
//! 160ms cadence (2560 samples at 16kHz).
//!
//! ## Weight bootstrap
//!
//! On first construction, [`ParakeetStt::new`] checks for the model
//! files in `weights_dir`. If any are missing it downloads them from
//! `altunenes/parakeet-rs` on Hugging Face Hub (subdir
//! `realtime_eou_120m-v1-onnx/`). Downloaded files:
//!
//! - `encoder.onnx` (~50MB)
//! - `decoder_joint.onnx` (~10MB)
//! - `tokenizer.json` (small)
//!
//! Total ~150MB cached locally. Default `weights_dir` is
//! `~/.cache/sensorium-voice/parakeet-eou/` (resolved via `$HOME`).
//!
//! ## Streaming protocol
//!
//! Per the `parakeet-rs` streaming example:
//!
//! 1. Buffer samples to `CHUNK_SIZE` (2560).
//! 2. Call `model.transcribe(&chunk, false)` per full chunk; the
//!    return is incremental text (may be empty for non-aligned
//!    frames). Append to the accumulated transcript.
//! 3. On flush: pad any residual buffer to a full chunk (zeros),
//!    transcribe; then send 3 zero-chunks to drain the model's
//!    internal state.
//! 4. The `Final` delta is the trimmed accumulated transcript.
//! 5. `reset()` clears our session state (buffer, accumulated
//!    transcript). `ParakeetEOU` itself exposes no `reset()` —
//!    its `transcribe(.., reset_on_eou)` flag is documented
//!    upstream as "not very reliable in practice", so v0.2 leaves
//!    the model's internal cache to drift across utterances and
//!    we rely on the EOU model's training to handle continuation.

use std::fs;
use std::path::{Path, PathBuf};

use parakeet_rs::ParakeetEOU;

use crate::backend::{SpeechToText, TranscriptDelta};
use crate::error::VoiceError;

/// Streaming chunk size: 160ms at 16kHz, per the EOU model's
/// configuration.
const CHUNK_SIZE: usize = 2560;

/// Hugging Face repository hosting the EOU model weights.
const HF_REPO: &str = "altunenes/parakeet-rs";

/// Subdirectory inside the HF repo where the EOU files live.
const HF_SUBDIR: &str = "realtime_eou_120m-v1-onnx";

/// File names the EOU model needs in `weights_dir`.
const EOU_FILES: &[&str] = &["encoder.onnx", "decoder_joint.onnx", "tokenizer.json"];

/// Real Parakeet EOU streaming backend.
pub struct ParakeetStt {
    model: ParakeetEOU,
    /// Accumulator for samples that have not yet filled a full
    /// `CHUNK_SIZE` chunk.
    buffer: Vec<f32>,
    /// Accumulated transcript across the current utterance.
    /// Cleared on `reset`; emitted as the `Final` delta on `flush`.
    accumulated: String,
    /// Tracks whether the partial text has grown since the last
    /// emit so we can avoid spamming empty deltas.
    last_emitted_len: usize,
    label: String,
}

impl ParakeetStt {
    /// Construct a Parakeet EOU backend, bootstrapping weights if
    /// they're not already present in `weights_dir`. If
    /// `weights_dir` is `None`, defaults to
    /// `~/.cache/sensorium-voice/parakeet-eou/`.
    pub fn new(weights_dir: Option<PathBuf>) -> Result<Self, VoiceError> {
        let weights_dir = weights_dir.unwrap_or_else(default_weights_dir);
        ensure_weights(&weights_dir)?;
        let weights_str = weights_dir.to_str().ok_or_else(|| {
            VoiceError::BackendSetup(format!("non-UTF-8 weights path: {}", weights_dir.display()))
        })?;
        let model = ParakeetEOU::from_pretrained(weights_str, None)
            .map_err(|e| VoiceError::BackendSetup(format!("ParakeetEOU::from_pretrained: {e}")))?;
        Ok(Self {
            model,
            buffer: Vec::with_capacity(CHUNK_SIZE * 2),
            accumulated: String::new(),
            last_emitted_len: 0,
            label: "parakeet-eou-120m".to_owned(),
        })
    }
}

impl SpeechToText for ParakeetStt {
    fn transcribe_chunk(&mut self, samples: &[f32]) -> Result<Option<TranscriptDelta>, VoiceError> {
        self.buffer.extend_from_slice(samples);
        let mut grew = false;
        while self.buffer.len() >= CHUNK_SIZE {
            let chunk: Vec<f32> = self.buffer.drain(..CHUNK_SIZE).collect();
            let text = self
                .model
                .transcribe(&chunk, false)
                .map_err(|e| VoiceError::Inference(format!("{e}")))?;
            if !text.is_empty() {
                self.accumulated.push_str(&text);
                grew = true;
            }
        }
        if grew && self.accumulated.len() != self.last_emitted_len {
            self.last_emitted_len = self.accumulated.len();
            Ok(Some(TranscriptDelta::Partial {
                text: self.accumulated.clone(),
            }))
        } else {
            Ok(None)
        }
    }

    fn flush(&mut self) -> Result<Option<TranscriptDelta>, VoiceError> {
        // Pad any residual buffer to a full chunk and transcribe.
        if !self.buffer.is_empty() {
            let mut chunk = std::mem::take(&mut self.buffer);
            chunk.resize(CHUNK_SIZE, 0.0);
            let text = self
                .model
                .transcribe(&chunk, false)
                .map_err(|e| VoiceError::Inference(format!("{e}")))?;
            self.accumulated.push_str(&text);
        }
        // Drain the model's internal state with 3 zero-chunks (per
        // the parakeet-rs streaming example).
        let zeros = vec![0.0_f32; CHUNK_SIZE];
        for _ in 0..3 {
            let text = self
                .model
                .transcribe(&zeros, false)
                .map_err(|e| VoiceError::Inference(format!("{e}")))?;
            if !text.is_empty() {
                self.accumulated.push_str(&text);
            }
        }
        let final_text = self.accumulated.trim().to_owned();
        Ok(Some(TranscriptDelta::Final { text: final_text }))
    }

    fn reset(&mut self) {
        // Our session-level state. ParakeetEOU itself has no public
        // `reset()`; its internal cache is best cleared by passing
        // `reset_on_eou=true` to `transcribe()` on the final chunk
        // of an utterance. Per the parakeet-rs streaming example,
        // even that flag is documented as "not very reliable in
        // practice," so v0.2 just rebuilds the surrounding state
        // and leaves the model's internal cache to drift across
        // utterances.
        self.buffer.clear();
        self.accumulated.clear();
        self.last_emitted_len = 0;
    }

    fn label(&self) -> &str {
        &self.label
    }
}

// --- Weight bootstrap ------------------------------------------------------

fn default_weights_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_owned());
    PathBuf::from(home)
        .join(".cache")
        .join("sensorium-voice")
        .join("parakeet-eou")
}

/// Verify all expected files exist in `dir`, downloading missing
/// ones from the canonical HF repo. Errors with
/// `VoiceError::WeightDownload` if download fails.
fn ensure_weights(dir: &Path) -> Result<(), VoiceError> {
    fs::create_dir_all(dir)
        .map_err(|e| VoiceError::WeightDownload(format!("create dir {}: {e}", dir.display())))?;
    let missing: Vec<&str> = EOU_FILES
        .iter()
        .copied()
        .filter(|f| !dir.join(f).exists())
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    eprintln!(
        "sensorium-voice: bootstrapping Parakeet EOU weights ({} missing files) from {}/{}/ — first run only, ~150MB",
        missing.len(),
        HF_REPO,
        HF_SUBDIR,
    );
    let api = hf_hub::api::sync::Api::new()
        .map_err(|e| VoiceError::WeightDownload(format!("hf-hub init: {e}")))?;
    let repo = api.model(HF_REPO.to_owned());
    for file in missing {
        let remote_path = format!("{HF_SUBDIR}/{file}");
        let cached = repo
            .get(&remote_path)
            .map_err(|e| VoiceError::WeightDownload(format!("download {remote_path}: {e}")))?;
        let target = dir.join(file);
        // hf-hub caches to its own location; copy into our weights
        // directory so `from_pretrained` finds everything together.
        fs::copy(&cached, &target).map_err(|e| {
            VoiceError::WeightDownload(format!(
                "copy {} → {}: {e}",
                cached.display(),
                target.display()
            ))
        })?;
    }
    Ok(())
}
