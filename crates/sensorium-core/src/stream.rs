//! Streaming substrate primitive: [`StreamUpdate`].
//!
//! A `StreamUpdate<T>` is what every streaming Sensorium producer emits
//! along its stream — voice STT partials, BCI confidence frames, gaze
//! samples — and what every consumer downstream of the substrate receives.
//!
//! Three variants, all carrying a [`Generation`] tag:
//!
//! - [`StreamUpdate::Partial`] — speculative value. May be revised by a
//!   later `Partial` with the same generation, or committed by a `Final`,
//!   or abandoned by `Cancelled`. Downstream stages may act on partials
//!   speculatively (low latency!) but must be prepared to roll back.
//!
//! - [`StreamUpdate::Final`] — committed value for this generation. No
//!   further updates with this generation are valid. Downstream stages
//!   may graduate any speculative work that was tagged with this
//!   generation to "ratified".
//!
//! - [`StreamUpdate::Cancelled`] — this generation is abandoned. No
//!   `Final` will arrive. Downstream stages must drop pending speculative
//!   work tagged with this generation. Typical triggers: user
//!   interruption (barge-in), end-of-session, upstream error.
//!
//! ## Why three (and not four)
//!
//! "Revised" would be the natural fourth — a partial that explicitly
//! supersedes the prior partial. We deliberately fold revision into
//! `Partial`: a later `Partial` with the same generation supersedes the
//! earlier one. This keeps the substrate small and the consumer's state
//! machine simple ("the latest `Partial(g)` is the current hypothesis at
//! generation `g`").
//!
//! Adding a variant here is a breaking change — same reasoning as
//! [`crate::PrimitiveKind`]. Pattern matches on `StreamUpdate` *should*
//! break if the design shifts.

use serde::{Deserialize, Serialize};

use crate::generation::Generation;

/// A single update along a generation-tagged stream.
///
/// See module docs for the full semantics. The minimal pattern for a
/// downstream consumer:
///
/// ```rust
/// use sensorium_core::{Generation, StreamUpdate};
///
/// fn handle(update: StreamUpdate<String>) {
///     match update {
///         StreamUpdate::Partial { generation, value } => {
///             // act speculatively; tag derived work with `generation`
///             let _ = (generation, value);
///         }
///         StreamUpdate::Final { generation, value } => {
///             // graduate any speculative work tagged with `generation`
///             let _ = (generation, value);
///         }
///         StreamUpdate::Cancelled { generation } => {
///             // drop speculative work tagged with `generation`
///             let _ = generation;
///         }
///     }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StreamUpdate<T> {
    /// Speculative value. May be revised by a later `Partial` with the
    /// same generation, committed by a `Final`, or abandoned by `Cancelled`.
    Partial {
        /// Generation this value belongs to.
        generation: Generation,
        /// The (speculative) value.
        value: T,
    },
    /// Committed value for this generation.
    Final {
        /// Generation this value belongs to.
        generation: Generation,
        /// The committed value.
        value: T,
    },
    /// This generation is abandoned. No `Final` will arrive.
    Cancelled {
        /// The abandoned generation.
        generation: Generation,
    },
}

impl<T> StreamUpdate<T> {
    /// The generation this update belongs to.
    ///
    /// Defined for every variant so callers can route without a
    /// pattern match.
    pub fn generation(&self) -> Generation {
        match self {
            Self::Partial { generation, .. }
            | Self::Final { generation, .. }
            | Self::Cancelled { generation } => *generation,
        }
    }

    /// `true` if this is a `Partial`.
    pub fn is_partial(&self) -> bool {
        matches!(self, Self::Partial { .. })
    }

    /// `true` if this is a `Final`.
    pub fn is_final(&self) -> bool {
        matches!(self, Self::Final { .. })
    }

    /// `true` if this is a `Cancelled`.
    pub fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled { .. })
    }

    /// Borrowed access to the value, or `None` for `Cancelled`.
    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Partial { value, .. } | Self::Final { value, .. } => Some(value),
            Self::Cancelled { .. } => None,
        }
    }

    /// Consume the update, returning the value or `None` for `Cancelled`.
    #[must_use]
    pub fn into_value(self) -> Option<T> {
        match self {
            Self::Partial { value, .. } | Self::Final { value, .. } => Some(value),
            Self::Cancelled { .. } => None,
        }
    }

    /// Transform the value (if any), preserving variant and generation.
    ///
    /// Functorial: `update.map(f).map(g) == update.map(|x| g(f(x)))`. A
    /// `Cancelled` carries no value and is returned unchanged (with the
    /// same generation).
    #[must_use]
    pub fn map<U, F>(self, f: F) -> StreamUpdate<U>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            Self::Partial { generation, value } => StreamUpdate::Partial {
                generation,
                value: f(value),
            },
            Self::Final { generation, value } => StreamUpdate::Final {
                generation,
                value: f(value),
            },
            Self::Cancelled { generation } => StreamUpdate::Cancelled { generation },
        }
    }
}
