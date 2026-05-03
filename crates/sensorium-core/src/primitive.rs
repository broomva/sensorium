//! The seven-primitive taxonomy.
//!
//! From `MIL-PROJECT.md` §5: every human-agent interaction decomposes into
//! combinations of seven primitives. Producers stamp this on every emitted
//! token; consumers route by it.
//!
//! Three of the seven are *passive* (Reference, Attention, State); they
//! arrive without the user actively expressing them. One (Predication) is
//! the only open-ended primitive needing model interpretation. The other
//! three (Modulation, Relation, Approval) are active but structurally
//! constrained.

use serde::{Deserialize, Serialize};

/// One of the seven MIL primitive kinds.
///
/// **Adding a variant is a breaking change.** The seven-element design is
/// load-bearing — the routing logic and the channel assignments are
/// indexed on this enum. If you find yourself wanting to add an eighth
/// primitive, the right move is a `MIL-PROJECT.md` design discussion, not
/// a quick enum extension.
///
/// We deliberately omit `#[non_exhaustive]` here for the same reason — we
/// *want* downstream pattern matches on this type to break if the design
/// shifts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PrimitiveKind {
    /// Picks out an entity. *Passive* — gaze + workspace context, no
    /// active expression required. Native channel: gaze fixation.
    Reference,
    /// Says what about the referent. *Active*, open-ended. Native
    /// channel: speech / typing. The only primitive needing a language
    /// model.
    Predication,
    /// Adjusts how (carefully, urgently, much). *Active, lightweight*.
    /// Native channel: continuous gesture parameters (velocity, pinch
    /// tension, repetition).
    Modulation,
    /// Connects multiple referents. *Active*. Native channel: spatial /
    /// structural gesture.
    Relation,
    /// Yes / no / undo / retry. *Active, binary*. Native channel: single
    /// discrete gesture or hotkey. Safety-critical, deliberately the
    /// simplest channel.
    Approval,
    /// What the user is focused on. *Passive*. Native channel: gaze
    /// fixation. Distinct from `Reference`: attention is the *gaze
    /// signal itself*; reference is what that signal *picks out*.
    Attention,
    /// Cognitive load, urgency, fatigue, mood. *Passive*. Native
    /// channels: biometrics, posture, rhythm.
    State,
}

impl PrimitiveKind {
    /// All seven primitives, in their canonical order.
    pub const ALL: [Self; 7] = [
        Self::Reference,
        Self::Predication,
        Self::Modulation,
        Self::Relation,
        Self::Approval,
        Self::Attention,
        Self::State,
    ];

    /// `true` if the user does not actively produce this primitive — the
    /// substrate observes it without effort. Reference, Attention, and
    /// State are passive (3 of 7).
    #[must_use]
    pub fn is_passive(self) -> bool {
        matches!(self, Self::Reference | Self::Attention | Self::State)
    }

    /// `true` if a language model is required to interpret values of this
    /// kind. Only `Predication` qualifies.
    #[must_use]
    pub fn requires_language_model(self) -> bool {
        matches!(self, Self::Predication)
    }

    /// `true` if the value is binary or near-binary (yes/no/cancel),
    /// safety-critical, and routed through the simplest channel. Only
    /// `Approval` qualifies.
    #[must_use]
    pub fn is_binary_safety_critical(self) -> bool {
        matches!(self, Self::Approval)
    }
}
