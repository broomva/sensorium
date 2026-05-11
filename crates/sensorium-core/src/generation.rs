//! Generation counters for streaming, speculative substrate values.
//!
//! Voice agents, BCI front-ends, and any future streaming sensor in the
//! Sensorium stack are *real-time incremental computation pipelines*. The
//! mic stream feeds STT partials, which feed speculative directives, which
//! feed an LLM, which feeds streaming TTS, which feeds the speaker. At every
//! arrow, *the upstream may revise its mind*: STT swaps a homophone, the
//! user interrupts, an LLM speculation is rejected.
//!
//! The substrate needs a way to say "this value belongs to *this* upstream
//! decision; if that decision is revised, downstream work tagged with it
//! must be dropped". [`Generation`] is that tag — a monotonic counter,
//! per-producer, threaded through every derived value.
//!
//! ## Lifecycle
//!
//! Each producer (e.g. one [`crate::SensorId`]'s `ParakeetStt` instance)
//! owns a single [`GenerationSeq`]. Each fresh "turn" — a new VAD onset,
//! a new keypress-to-talk cycle — calls [`GenerationSeq::advance`] to mint
//! a new generation. All `Partial`, `Final`, and downstream-derived values
//! emitted between that and the next `advance()` share the generation.
//!
//! ## Pairing with [`crate::SensorId`]
//!
//! Generation numbers are *only* unique within a single producer. Two
//! independent producers can mint the same generation number. Consumers
//! that demux multiple producers should always carry the
//! `(SensorId, Generation)` tuple — provenance already holds the sensor.

use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

/// Monotonic stream-update generation within a single producer.
///
/// `Generation` is a `Copy` newtype around `u64`. It serializes
/// transparently — a `Generation(7)` is encoded as `7` on the wire. This
/// keeps the substrate's wire format compact and lets external schemas
/// model the field as a plain integer.
///
/// Generations are minted by [`GenerationSeq::advance`]; the constructor
/// [`Generation::new`] exists primarily for tests and replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Generation(u64);

impl Generation {
    /// The first generation a fresh [`GenerationSeq`] hands out.
    ///
    /// Producers may use `INITIAL` as a sentinel for "no turn has started
    /// yet" — but doing so means they cannot also emit a real turn at
    /// generation 0. Prefer minting from a `GenerationSeq` in production.
    pub const INITIAL: Self = Self(0);

    /// Construct a `Generation` from a raw counter value.
    ///
    /// Intended for tests, replay, and snapshot deserialization. Live
    /// producers should mint via [`GenerationSeq::advance`].
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Raw counter value.
    #[must_use]
    pub const fn into_inner(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for Generation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Per-producer minter of monotonic [`Generation`] counters.
///
/// Construct one per producer instance. Each call to
/// [`GenerationSeq::advance`] returns a strictly-larger generation than
/// the last; [`GenerationSeq::current`] peeks at the next value without
/// advancing.
///
/// Thread-safe by construction (`AtomicU64` with `Relaxed` ordering —
/// the only happens-before relationship we need is monotonicity within
/// the sequence, not cross-thread synchronization of the data each
/// generation tags).
#[derive(Debug)]
pub struct GenerationSeq {
    next: AtomicU64,
}

impl GenerationSeq {
    /// A fresh sequence whose next `advance()` returns [`Generation::INITIAL`].
    #[must_use]
    pub const fn new() -> Self {
        Self {
            next: AtomicU64::new(0),
        }
    }

    /// Mint a new generation, advancing the sequence.
    ///
    /// Each call returns a value strictly greater than every prior return.
    /// Safe to call concurrently from multiple threads.
    pub fn advance(&self) -> Generation {
        Generation(self.next.fetch_add(1, Ordering::Relaxed))
    }

    /// The generation that the *next* `advance()` will return.
    ///
    /// Does not advance the sequence. Equal to [`Generation::INITIAL`] for
    /// a fresh sequence.
    pub fn current(&self) -> Generation {
        Generation(self.next.load(Ordering::Relaxed))
    }
}

impl Default for GenerationSeq {
    fn default() -> Self {
        Self::new()
    }
}
