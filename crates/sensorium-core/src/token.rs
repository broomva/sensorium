//! `PrimitiveToken` — the event taxonomy producers emit and Pneuma consumes.
//!
//! A `PrimitiveToken` is one observation, of one primitive kind, with full
//! provenance. The seven variants of [`PrimitiveToken`] correspond exactly
//! to the seven primitives in `MIL-PROJECT.md` §5; the discriminant of the
//! enum **is** the [`crate::primitive::PrimitiveKind`].
//!
//! ## Design choice: enum, not trait
//!
//! We model the token taxonomy as a closed enum rather than a trait object
//! for two reasons:
//!
//! 1. **The seven primitives are load-bearing.** The architecture is built
//!    on the claim that *exactly seven* primitives partition the design
//!    space. An enum makes that claim visible to the type system; a trait
//!    object hides it behind a dyn-dispatched ABI.
//! 2. **Pneuma's binder pattern-matches on this enum.** Trait dispatch
//!    would force every consumer through a virtual call on the hot path;
//!    the closed enum lets the compiler optimize the dispatch.
//!
//! ## Active vs. passive variants
//!
//! Three variants ([`PrimitiveToken::Reference`], [`PrimitiveToken::Attention`],
//! [`PrimitiveToken::State`]) carry passively-observed values. Four variants
//! ([`PrimitiveToken::Predication`], [`PrimitiveToken::Modulation`],
//! [`PrimitiveToken::Relation`], [`PrimitiveToken::Approval`]) carry actively-
//! produced values. The substrate doesn't enforce this distinction (a
//! producer can mis-stamp a passive event as active); the test suite
//! verifies that producer-supplied tokens carry consistent
//! [`crate::Provenance::primitive`] discriminants.

use serde::{Deserialize, Serialize};

use crate::attention::GazeFixation;
use crate::biometric::{ArousalLevel, BiometricSnapshot};
use crate::entity::{AppId, FileRef, SelectionRef, SymbolRef, Uri, WindowId};
use crate::posture::PostureSnapshot;
use crate::primitive::PrimitiveKind;
use crate::provenance::Tagged;

// --- Approval / Relation / Modulation / Attention sub-enums -----------------

/// What approval was expressed.
///
/// Six discrete moves — the safety-critical primitive's full vocabulary
/// (`MIL-PROJECT.md` §14, "Engage / commit / cancel / approve / reject /
/// undo"). Each maps to a default gesture *and* a default hotkey;
/// producers stamp whichever they observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ApprovalEvent {
    /// "I am about to start a directive." Held pinch / hotkey-press.
    Engage,
    /// "Commit the directive currently composing." Pinch-release / Enter.
    Commit,
    /// "Cancel the directive currently composing." Open palm / Esc.
    Cancel,
    /// "Approve the proposed directive." Pinch / Enter on a Proposal.
    Approve,
    /// "Reject the proposed directive." Open palm / Esc on a Proposal.
    Reject,
    /// "Undo the most recent committed directive." Flick / Cmd-Z.
    Undo,
}

/// What spatial / structural relation was observed.
///
/// `MIL-PROJECT.md` §5.1: Relation is "small classifier or geometric
/// heuristics on landmark windows. ~10 patterns." We seed the enum with
/// the patterns the spec implies; producers may surface fewer if their
/// recognizer is simpler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum RelationEvent {
    /// Connect the active referent to the next: "this *and* that".
    And,
    /// Choose between referents: "this *or* that".
    Or,
    /// Group referents under a parent: "*these things together*".
    Group,
    /// Sequence referents: "this *then* that".
    Sequence,
    /// Direction of action — point-from a referent.
    From,
    /// Direction of action — point-to a referent.
    To,
    /// Container relation: "*inside* this".
    Inside,
    /// Container relation: "*outside* this".
    Outside,
    /// Pair / mirror: "the same on the other one".
    Mirror,
    /// Free-form — the producer recognized a relation outside the
    /// canonical set. Carries a free-form tag for forward compatibility.
    Custom(u32),
}

/// A continuous-parameter modulation observation.
///
/// All values normalized to `[-1.0, 1.0]` or `[0.0, 1.0]` per the
/// `parameter` enum. Producers compute these from gestures the user is
/// already making — no separate gesture vocabulary required.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ModulationEvent {
    /// Which modulation parameter.
    pub parameter: ModulationParameter,
    /// Normalized value. Range depends on `parameter`; see the variant
    /// docs.
    pub value: f32,
}

/// The modulation parameters the substrate recognizes.
///
/// One discriminator per `Modifier` variant in `pneuma-core::Modifier`,
/// minus the structural modifiers (`TimeWindow`, `Custom`) that don't have
/// a continuous-parameter source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ModulationParameter {
    /// "How much" — magnitude, `[0.0, 1.0]`.
    Magnitude,
    /// "How careful" — carefulness, `[0.0, 1.0]`. *Tightens* downstream
    /// confidence thresholds.
    Carefulness,
    /// "How urgent" — urgency, `[0.0, 1.0]`. *Shortens ratify dwell only*;
    /// does **not** lower confidence thresholds.
    Urgency,
    /// "How committed" — commitment / pinch tension, `[0.0, 1.0]`.
    Commitment,
    /// Abstraction level (high gesture height vs low), `[0.0, 1.0]`.
    AbstractionLevel,
    /// Distributivity (single repetition tap vs sustained pulses),
    /// `[0.0, 1.0]`.
    Distributive,
}

/// What the substrate observed about user attention.
///
/// Distinct from [`PrimitiveToken::Reference`]: attention is the *gaze
/// signal itself*; reference is what that signal *picks out*. Pneuma's
/// binder converts attention to reference by hit-testing against the
/// workspace context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AttentionEvent {
    /// User fixated on a region.
    Fixation(GazeFixation),
    /// User looked away from the device entirely. Pneuma should *not*
    /// proceed with optimistic dispatch while this is the active
    /// attention state.
    LookAway,
    /// User's gaze re-engaged after a `LookAway`.
    Reengage,
}

// --- PrimitiveToken (the closed enum) ----------------------------------------

/// A single observation of one of the seven primitives.
///
/// Each variant carries its primitive payload as a [`Tagged<T>`] — full
/// provenance, calibration, and privacy tier. The discriminant of this enum
/// must match `Tagged::provenance.primitive`; the substrate offers
/// [`PrimitiveToken::expected_kind`] for that consistency check.
///
/// **Adding a variant is a breaking change.** Same reasoning as
/// [`PrimitiveKind`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PrimitiveToken {
    /// Reference token — picks out an entity. The payload is the resolved
    /// referent kind. Producers that emit this have already done a
    /// hit-test; if the substrate has not yet resolved the reference,
    /// emit an [`PrimitiveToken::Attention`] token instead.
    Reference(Tagged<ReferentObservation>),
    /// Predication token — open-ended utterance content. The payload is
    /// the raw transcribed text; Pneuma's predication-model adapter
    /// interprets it.
    Predication(Tagged<String>),
    /// Modulation token — continuous parameter on an active gesture.
    Modulation(Tagged<ModulationEvent>),
    /// Relation token — spatial or structural connection between
    /// referents.
    Relation(Tagged<RelationEvent>),
    /// Approval token — engage / commit / cancel / approve / reject / undo.
    Approval(Tagged<ApprovalEvent>),
    /// Attention token — gaze signal itself, possibly resolved.
    Attention(Tagged<AttentionEvent>),
    /// State token — biometric / posture / arousal observation.
    State(Tagged<StateObservation>),
}

impl PrimitiveToken {
    /// The [`PrimitiveKind`] this token represents.
    ///
    /// Always equals `self.provenance().primitive` for a well-formed token.
    /// Mismatches are sensor bugs — see [`PrimitiveToken::is_well_formed`].
    #[must_use]
    pub fn expected_kind(&self) -> PrimitiveKind {
        match self {
            Self::Reference(_) => PrimitiveKind::Reference,
            Self::Predication(_) => PrimitiveKind::Predication,
            Self::Modulation(_) => PrimitiveKind::Modulation,
            Self::Relation(_) => PrimitiveKind::Relation,
            Self::Approval(_) => PrimitiveKind::Approval,
            Self::Attention(_) => PrimitiveKind::Attention,
            Self::State(_) => PrimitiveKind::State,
        }
    }

    /// Borrow the token's provenance, regardless of variant.
    #[must_use]
    pub fn provenance(&self) -> &crate::provenance::Provenance {
        match self {
            Self::Reference(t) => &t.provenance,
            Self::Predication(t) => &t.provenance,
            Self::Modulation(t) => &t.provenance,
            Self::Relation(t) => &t.provenance,
            Self::Approval(t) => &t.provenance,
            Self::Attention(t) => &t.provenance,
            Self::State(t) => &t.provenance,
        }
    }

    /// `true` iff the variant matches the provenance discriminant.
    ///
    /// Producers occasionally mis-stamp; the substrate provides this
    /// check so consumers can drop or repair malformed tokens at the
    /// boundary.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        self.provenance().primitive == self.expected_kind()
    }
}

// --- Observation payloads ----------------------------------------------------

/// What kind of entity a [`PrimitiveToken::Reference`] picks out.
///
/// Mirrors `pneuma_core::Referent`'s leaf cases (the substrate observes
/// the leaves; the directive's `Referent` enum extends them with `Set`,
/// `Range`, `Anaphor`, `Locus`, which are not direct sensor outputs).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReferentObservation {
    /// A file the substrate observed.
    File(FileRef),
    /// A selection (file + byte range) the substrate observed.
    Selection(SelectionRef),
    /// A window.
    Window(WindowId),
    /// An application.
    App(AppId),
    /// A code symbol surfaced by an LSP-equipped observer.
    Symbol(SymbolRef),
    /// A URL.
    Url(Uri),
}

/// What kind of state a [`PrimitiveToken::State`] reports.
///
/// Either a biometric snapshot, a posture snapshot, or an arousal-only
/// estimate from a producer that doesn't expose the underlying signals.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum StateObservation {
    /// Full biometric snapshot.
    Biometric(BiometricSnapshot),
    /// Full posture snapshot.
    Posture(PostureSnapshot),
    /// Arousal-only — used by producers that infer arousal from typing
    /// rhythm or other proxies without measuring biometrics directly.
    ArousalOnly(ArousalLevel),
}
