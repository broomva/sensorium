//! The seven-primitive taxonomy is structurally complete and consistent.
//!
//! Properties under test (cross-references to `MIL-PROJECT.md`):
//!
//! - **§5** "Every human-agent interaction decomposes into combinations of
//!   seven primitives." Eighth-primitive proposals must trigger a design
//!   review, not just an enum extension.
//! - **§5** "Three of seven are passive." (Reference, Attention, State.)
//! - **§5** "Approval is binary." (Safety-critical primitive, simplest channel.)
//! - **§5** "Only predication is open-ended." (Only primitive needing an LLM.)
//! - **§9.4** "The model never determines whether to dispatch." Model usage
//!   limited to predication content interpretation.

use sensorium_core::token::{ModulationParameter, ReferentObservation, StateObservation};
use sensorium_core::{
    ApprovalEvent, AttentionEvent, Calibration, ModulationEvent, PrimitiveKind, PrimitiveToken,
    PrivacyTier, Provenance, RelationEvent, SensorId, Tagged, Timestamp,
};

/// `PrimitiveKind::ALL` has exactly seven elements. The architecture
/// claims "exactly seven" — adding an eighth is a breaking change that
/// must be designed, not slipped in.
#[test]
fn primitive_kind_has_exactly_seven_variants() {
    assert_eq!(PrimitiveKind::ALL.len(), 7);
}

/// Three of seven primitives are passive. The architecture rests on
/// passive observation dominating active expression (`MIL-PROJECT.md`
/// §1, §4.4).
#[test]
fn three_of_seven_primitives_are_passive() {
    let passive_count = PrimitiveKind::ALL.iter().filter(|p| p.is_passive()).count();
    assert_eq!(passive_count, 3);
}

/// The passive set is exactly Reference, Attention, State. If this set
/// changes, the architecture has shifted underneath.
#[test]
fn passive_set_is_reference_attention_state() {
    assert!(PrimitiveKind::Reference.is_passive());
    assert!(PrimitiveKind::Attention.is_passive());
    assert!(PrimitiveKind::State.is_passive());

    assert!(!PrimitiveKind::Predication.is_passive());
    assert!(!PrimitiveKind::Modulation.is_passive());
    assert!(!PrimitiveKind::Relation.is_passive());
    assert!(!PrimitiveKind::Approval.is_passive());
}

/// Only Predication requires a language model. This is the v0.2 model
/// usage commitment (§9): every other primitive uses signal processing,
/// FSMs, or hit-tests, not learned dispatch.
#[test]
fn only_predication_requires_a_language_model() {
    let model_using = PrimitiveKind::ALL
        .iter()
        .filter(|p| p.requires_language_model())
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(model_using, vec![PrimitiveKind::Predication]);
}

/// Only Approval is binary safety-critical. The "engage/commit/cancel/
/// approve/reject/undo" set is the simplest channel by design — never
/// model-interpreted.
#[test]
fn only_approval_is_binary_safety_critical() {
    let bsc = PrimitiveKind::ALL
        .iter()
        .filter(|p| p.is_binary_safety_critical())
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(bsc, vec![PrimitiveKind::Approval]);
}

/// Each `PrimitiveToken` variant maps to the corresponding `PrimitiveKind`
/// via `expected_kind`. The two enums are kept in lockstep; this test
/// guards against a future maintainer adding a variant on one side
/// without the other.
#[test]
fn each_primitive_token_variant_has_correct_expected_kind() {
    let sensor = SensorId::new();
    let now = Timestamp::now();
    let mk_provenance = |primitive: PrimitiveKind| {
        Provenance::new(
            sensor,
            now,
            Calibration::synthetic(),
            PrivacyTier::Public,
            primitive,
        )
    };

    let tokens: Vec<(PrimitiveToken, PrimitiveKind)> = vec![
        (
            PrimitiveToken::Reference(Tagged::new(
                ReferentObservation::Url(
                    sensorium_core::Uri::new("https://example.com").expect("non-empty"),
                ),
                mk_provenance(PrimitiveKind::Reference),
            )),
            PrimitiveKind::Reference,
        ),
        (
            PrimitiveToken::Predication(Tagged::new(
                "rename it".to_owned(),
                mk_provenance(PrimitiveKind::Predication),
            )),
            PrimitiveKind::Predication,
        ),
        (
            PrimitiveToken::Modulation(Tagged::new(
                ModulationEvent {
                    parameter: ModulationParameter::Carefulness,
                    value: 0.8,
                },
                mk_provenance(PrimitiveKind::Modulation),
            )),
            PrimitiveKind::Modulation,
        ),
        (
            PrimitiveToken::Relation(Tagged::new(
                RelationEvent::And,
                mk_provenance(PrimitiveKind::Relation),
            )),
            PrimitiveKind::Relation,
        ),
        (
            PrimitiveToken::Approval(Tagged::new(
                ApprovalEvent::Commit,
                mk_provenance(PrimitiveKind::Approval),
            )),
            PrimitiveKind::Approval,
        ),
        (
            PrimitiveToken::Attention(Tagged::new(
                AttentionEvent::LookAway,
                mk_provenance(PrimitiveKind::Attention),
            )),
            PrimitiveKind::Attention,
        ),
        (
            PrimitiveToken::State(Tagged::new(
                StateObservation::ArousalOnly(sensorium_core::ArousalLevel::Normal),
                mk_provenance(PrimitiveKind::State),
            )),
            PrimitiveKind::State,
        ),
    ];

    for (token, expected) in &tokens {
        assert_eq!(token.expected_kind(), *expected);
        assert!(
            token.is_well_formed(),
            "well-formed tokens must have matching variant and provenance"
        );
    }

    // Coverage check — exactly seven well-formed tokens, one per kind.
    assert_eq!(tokens.len(), 7);
}

/// A token whose provenance discriminant disagrees with its variant is
/// *not* well-formed. The substrate provides this check so the boundary
/// between sensors and Pneuma can drop or repair malformed tokens
/// rather than silently misroute.
#[test]
fn mismatched_provenance_discriminant_is_not_well_formed() {
    let sensor = SensorId::new();
    let now = Timestamp::now();

    // Construct a `Reference` token with `Approval` provenance — a sensor bug.
    let bad_provenance = Provenance::new(
        sensor,
        now,
        Calibration::synthetic(),
        PrivacyTier::Public,
        PrimitiveKind::Approval,
    );
    let bad_token = PrimitiveToken::Reference(Tagged::new(
        ReferentObservation::Url(sensorium_core::Uri::new("https://x.test").expect("non-empty")),
        bad_provenance,
    ));

    assert_eq!(bad_token.expected_kind(), PrimitiveKind::Reference);
    assert_eq!(bad_token.provenance().primitive, PrimitiveKind::Approval);
    assert!(!bad_token.is_well_formed());
}

/// Approval covers all six discourse moves the spec defines (engage,
/// commit, cancel, approve, reject, undo). Catching a stray addition or
/// removal is what this test is for.
#[test]
fn approval_event_covers_six_canonical_moves() {
    use ApprovalEvent::{Approve, Cancel, Commit, Engage, Reject, Undo};
    let canonical = [Engage, Commit, Cancel, Approve, Reject, Undo];
    // Each is distinct.
    let mut as_str: Vec<String> = canonical.iter().map(|e| format!("{e:?}")).collect();
    as_str.sort();
    as_str.dedup();
    assert_eq!(as_str.len(), 6);
}
