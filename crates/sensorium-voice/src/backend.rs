//! `SpeechToText` backend trait + implementations.
//!
//! The trait abstracts over the STT engine (Mock for tests,
//! Parakeet for real inference). Implementations consume audio
//! chunks and emit `TranscriptDelta`s — incremental partial
//! transcripts that the session aggregates into final tokens.

use crate::error::VoiceError;

/// One incremental transcript update from the backend.
///
/// `Partial` deltas accumulate into the final transcript; backends
/// emit `Final` at end-of-utterance with the consolidated text.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TranscriptDelta {
    /// Partial transcript so far. May be empty if the backend hasn't
    /// emitted anything yet for this utterance.
    Partial {
        /// Text accumulated to this point.
        text: String,
    },
    /// Final transcript for the current utterance. Backends emit
    /// this when they detect end-of-utterance (or when the session
    /// drains them at session-end).
    Final {
        /// The full transcript.
        text: String,
    },
}

impl TranscriptDelta {
    /// Borrow the underlying text regardless of variant.
    #[must_use]
    pub fn text(&self) -> &str {
        match self {
            Self::Partial { text } | Self::Final { text } => text,
        }
    }

    /// `true` if this is a `Final` delta.
    #[must_use]
    pub fn is_final(&self) -> bool {
        matches!(self, Self::Final { .. })
    }
}

/// STT backend surface.
///
/// Implementations consume audio chunks (16kHz f32 mono) and emit
/// transcript deltas. Synchronous on purpose for v0.2: the inference
/// thread holds the backend and feeds it chunks one at a time.
///
/// For streaming-capable backends (Parakeet EOU), `transcribe_chunk`
/// returns `Partial` deltas mid-utterance and `Final` at the end.
/// For batch backends, all chunks accumulate into one `Final` at
/// `flush`.
pub trait SpeechToText: Send {
    /// Feed an audio chunk and (optionally) emit a delta. The chunk
    /// is `16kHz` f32 mono. Length is backend-dependent — Parakeet
    /// EOU prefers 2560 samples (160ms); the session normalizes.
    fn transcribe_chunk(&mut self, chunk: &[f32]) -> Result<Option<TranscriptDelta>, VoiceError>;

    /// Signal end-of-utterance. Backends use this to flush
    /// remaining state and emit a `Final` delta.
    fn flush(&mut self) -> Result<Option<TranscriptDelta>, VoiceError>;

    /// Reset internal state for the next utterance. Called by the
    /// session after `flush`.
    fn reset(&mut self) {
        // Default implementation is a no-op for stateless backends.
    }

    /// Identifier for the journal / HUD. Examples: "parakeet-eou",
    /// "mock", "whisper-large-v3".
    fn label(&self) -> &str;
}

// --- MockStt ---------------------------------------------------------------

/// Scripted-response backend for tests. Holds a queue of canned
/// transcripts; emits each one as a `Final` delta on `flush`. Ignores
/// audio chunks (returns `None`).
///
/// When the queue drains, subsequent flushes repeat the last
/// response — useful for tests that flush more than they primed.
#[derive(Debug, Clone)]
pub struct MockStt {
    canned: std::collections::VecDeque<String>,
    /// Cached last-emitted response so we can repeat after queue
    /// drains rather than emitting empty strings.
    last: String,
    label: String,
}

impl MockStt {
    /// Construct from a list of canned responses.
    #[must_use]
    pub fn new(canned: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let queue: std::collections::VecDeque<String> =
            canned.into_iter().map(Into::into).collect();
        let last = queue.back().cloned().unwrap_or_default();
        Self {
            canned: queue,
            last,
            label: "mock".to_owned(),
        }
    }
}

impl SpeechToText for MockStt {
    fn transcribe_chunk(&mut self, _chunk: &[f32]) -> Result<Option<TranscriptDelta>, VoiceError> {
        // Mock backends emit only on flush.
        Ok(None)
    }

    fn flush(&mut self) -> Result<Option<TranscriptDelta>, VoiceError> {
        let text = if let Some(next) = self.canned.pop_front() {
            self.last.clone_from(&next);
            next
        } else {
            self.last.clone()
        };
        Ok(Some(TranscriptDelta::Final { text }))
    }

    fn label(&self) -> &str {
        &self.label
    }
}
