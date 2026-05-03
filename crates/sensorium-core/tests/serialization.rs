//! Serialization round-trips for the Public-tier substrate types.
//!
//! Properties under test:
//!
//! - **§7.1** Sensorium subsystems exchange substrate state across
//!   process boundaries (`sensorium-context` → `sensorium-lago`,
//!   bridge to Pneuma). Public-tier types must round-trip cleanly.
//! - **§10.3** "Round-trip through serde_json" — same property here.
//! - **Privacy-tier separation**: types containing or wrapping
//!   `LocalOnly` cannot be serialized; those tests live in
//!   `privacy_markers.rs`.
//!
//! What we test here are the *substrate primitives* — IDs, calibration,
//! provenance, primitive tokens — and the wire format we'll journal.

use sensorium_core::attention::Fixation;
use sensorium_core::posture::PostureSnapshot;
use sensorium_core::token::{ModulationParameter, ReferentObservation, StateObservation};
use sensorium_core::{
    ActivityMarker, ApprovalEvent, ArousalLevel, AttentionEvent, BiometricSnapshot, Calibration,
    CognitiveLoad, FileRef, GazeFixation, GazePoint, ModulationEvent, PrimitiveKind,
    PrimitiveToken, PrivacyTier, Provenance, RecentActivity, RingBuffer, SelectionRef, SensorId,
    SensorKind, SensorMetadata, Tagged, TextSpan, Timestamp, UserState, WorkspaceState,
};

/// `Calibration` round-trips through serde_json.
#[test]
fn calibration_round_trips_through_json() {
    let original = Calibration::calibrated_now();
    let json = serde_json::to_string(&original).expect("serialize");
    let de: Calibration = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(de, original);
}

/// `Provenance` round-trips. This is the audit-trail metadata that
/// the substrate journals; the wire format is critical.
#[test]
fn provenance_round_trips_through_json() {
    let original = Provenance::new(
        SensorId::new(),
        Timestamp::now(),
        Calibration::synthetic(),
        PrivacyTier::Public,
        PrimitiveKind::Reference,
    );
    let json = serde_json::to_string(&original).expect("serialize");
    let de: Provenance = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(de, original);
}

/// `Tagged<T>` round-trips when `T` does. This is the universal-currency
/// wrapper; everything in the substrate is `Tagged<T>`, so journal
/// fidelity depends on this property.
#[test]
fn tagged_round_trips_when_inner_does() {
    let provenance = Provenance::new(
        SensorId::new(),
        Timestamp::now(),
        Calibration::calibrated_now(),
        PrivacyTier::Public,
        PrimitiveKind::Reference,
    );
    let original = Tagged::new("hello".to_owned(), provenance);

    let json = serde_json::to_string(&original).expect("serialize");
    let de: Tagged<String> = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(de.value, "hello");
    assert_eq!(de.provenance, provenance);
}

/// `RingBuffer<T, N>` round-trips. The custom serde impl serializes as
/// a flat sequence in oldest-first order; deserialization rebuilds the
/// ring via push.
#[test]
fn ring_buffer_round_trips_with_custom_serde() {
    let mut original: RingBuffer<u32, 4> = RingBuffer::new();
    for n in 1..=6 {
        original.push(n);
    }
    // After 6 pushes into a capacity-4 ring: contents should be [3, 4, 5, 6].

    let json = serde_json::to_string(&original).expect("serialize");
    assert_eq!(json, "[3,4,5,6]", "wire format is the iteration order");

    let de: RingBuffer<u32, 4> = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(de.len(), 4);
    let collected: Vec<u32> = de.iter().copied().collect();
    assert_eq!(collected, vec![3, 4, 5, 6]);
}

/// Deserializing a sequence longer than capacity overflows just like
/// `push` — leading items are evicted. This matches the producer
/// pattern: a long stream of activity always lands in a bounded ring.
#[test]
fn ring_buffer_deserialization_overflows_via_push() {
    let json = "[10, 20, 30, 40, 50]";
    let de: RingBuffer<u32, 3> = serde_json::from_str(json).expect("deserialize");
    let collected: Vec<u32> = de.iter().copied().collect();
    assert_eq!(collected, vec![30, 40, 50]);
}

/// `RecentActivity` (the substrate's recent-activity log) round-trips.
#[test]
fn recent_activity_round_trips() {
    let mut activity = RecentActivity::empty();
    activity.ring.push(ActivityMarker::FileAccessed {
        file: FileRef::new("/tmp/x.txt"),
        at: Timestamp::now(),
    });
    activity.ring.push(ActivityMarker::DirectiveCommitted {
        directive_id: "abc".to_owned(),
        at: Timestamp::now(),
    });

    let json = serde_json::to_string(&activity).expect("serialize");
    let de: RecentActivity = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(de.ring.len(), 2);
}

/// `WorkspaceState` round-trips. The `Arc<>` lives at the
/// `WorkspaceContext`/`WorkspaceSnapshot` level, not inside the state
/// itself, so serialization is straightforward.
#[test]
fn workspace_state_round_trips() {
    let now = Timestamp::now();
    let original = WorkspaceState::neutral(now);

    let json = serde_json::to_string(&original).expect("serialize");
    let de: WorkspaceState = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(de, original);
}

/// `WorkspaceState` with non-trivial fields populated round-trips.
#[test]
fn workspace_state_with_data_round_trips() {
    let now = Timestamp::now();
    let mut state = WorkspaceState::neutral(now);
    state.focused_window = Some(sensorium_core::WindowId::new("win-42").expect("id"));
    state.focused_app = Some(sensorium_core::AppId::new("com.test.app").expect("id"));
    state.visible_files.push(FileRef::new("/etc/hosts"));
    state
        .sensors
        .push(SensorMetadata::new(SensorKind::Workspace));
    state.user_state = UserState {
        biometric: BiometricSnapshot::neutral(now),
        posture: PostureSnapshot::unknown(now),
        cognitive_load: CognitiveLoad::Engaged,
        at: now,
    };

    let json = serde_json::to_string(&state).expect("serialize");
    let de: WorkspaceState = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(de, state);
}

/// All seven `PrimitiveToken` variants round-trip through serde_json.
/// The wire format is the journal format; faithfulness here is what
/// makes Lago replay possible.
#[test]
fn all_primitive_token_variants_round_trip() {
    let now = Timestamp::now();
    let mk_provenance = |primitive: PrimitiveKind| {
        Provenance::new(
            SensorId::new(),
            now,
            Calibration::synthetic(),
            PrivacyTier::Public,
            primitive,
        )
    };

    let tokens = vec![
        PrimitiveToken::Reference(Tagged::new(
            ReferentObservation::File(FileRef::new("/tmp/x.txt")),
            mk_provenance(PrimitiveKind::Reference),
        )),
        PrimitiveToken::Predication(Tagged::new(
            "rename it".to_owned(),
            mk_provenance(PrimitiveKind::Predication),
        )),
        PrimitiveToken::Modulation(Tagged::new(
            ModulationEvent {
                parameter: ModulationParameter::Carefulness,
                value: 0.5,
            },
            mk_provenance(PrimitiveKind::Modulation),
        )),
        PrimitiveToken::Relation(Tagged::new(
            sensorium_core::RelationEvent::Group,
            mk_provenance(PrimitiveKind::Relation),
        )),
        PrimitiveToken::Approval(Tagged::new(
            ApprovalEvent::Commit,
            mk_provenance(PrimitiveKind::Approval),
        )),
        PrimitiveToken::Attention(Tagged::new(
            AttentionEvent::Fixation(GazeFixation::new(Fixation::new(
                GazePoint::new(100.0, 200.0),
                400,
                now,
            ))),
            mk_provenance(PrimitiveKind::Attention),
        )),
        PrimitiveToken::State(Tagged::new(
            StateObservation::ArousalOnly(ArousalLevel::Normal),
            mk_provenance(PrimitiveKind::State),
        )),
    ];

    for token in &tokens {
        let json = serde_json::to_string(token).expect("primitive token serialize");
        let de: PrimitiveToken = serde_json::from_str(&json).expect("primitive token deserialize");
        assert_eq!(&de, token);
    }
}

/// Selection / span / file references round-trip.
#[test]
fn selection_ref_round_trips() {
    let original = SelectionRef::new(
        FileRef::new("/tmp/code.rs"),
        TextSpan::new(10, 50).expect("valid"),
    );
    let json = serde_json::to_string(&original).expect("serialize");
    let de: SelectionRef = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(de, original);
}
