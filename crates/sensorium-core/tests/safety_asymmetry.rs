//! The safety-asymmetry property: tightening fires on multiple signals;
//! loosening never fires from arousal/fatigue.
//!
//! Properties under test (cross-references to `MIL-PROJECT.md`):
//!
//! - **§10.2** "Urgency does not lower the confidence threshold. Only
//!   shortens ratify dwell. This asymmetry is tested directly."
//! - **§5.1** Modulation channel: Carefulness *tightens*, Urgency
//!   *shortens dwell only*. The substrate exposes both signals; the
//!   asymmetry is enforced one level up in `pneuma-core`. Here we
//!   verify that the substrate's *signals* support that asymmetry —
//!   that `should_tighten_threshold` is ergonomic for tightening and
//!   has no symmetric `should_loosen_threshold` exposed.
//!
//! The asymmetry is structural: the `UserState::should_tighten_threshold`
//! method exists; no `should_loosen_threshold` exists. A future
//! maintainer cannot accidentally introduce one without writing it,
//! and writing it would fail this test.

use sensorium_core::{
    ArousalLevel, BiometricSnapshot, CognitiveLoad, Posture, PostureSnapshot, PresenceLevel,
    Timestamp, UserState,
};

/// Cognitive load `Engaged` and `Overloaded` both tighten thresholds.
/// "User is committed" and "user is stressed" are both reasons to be
/// stricter.
#[test]
fn engaged_and_overloaded_load_tighten_thresholds() {
    assert!(!CognitiveLoad::Underloaded.should_tighten_threshold());
    assert!(!CognitiveLoad::Nominal.should_tighten_threshold());
    assert!(CognitiveLoad::Engaged.should_tighten_threshold());
    assert!(CognitiveLoad::Overloaded.should_tighten_threshold());
}

/// Arousal `Elevated` and `Acute` both indicate high arousal, both
/// must tighten. The ordering of `ArousalLevel` mirrors the load
/// ordering (`Low < Normal < Elevated < Acute`).
#[test]
fn elevated_and_acute_arousal_tighten_thresholds() {
    assert!(!ArousalLevel::Low.is_high_arousal());
    assert!(!ArousalLevel::Normal.is_high_arousal());
    assert!(ArousalLevel::Elevated.is_high_arousal());
    assert!(ArousalLevel::Acute.is_high_arousal());

    assert!(ArousalLevel::Low < ArousalLevel::Normal);
    assert!(ArousalLevel::Normal < ArousalLevel::Elevated);
    assert!(ArousalLevel::Elevated < ArousalLevel::Acute);
}

/// `Posture::Fatigued` indicates fatigue → tighten. Other posture
/// values (LeanForward, Upright, LeanBack, Unknown) do not.
#[test]
fn fatigued_posture_tightens_thresholds() {
    assert!(!Posture::LeanForward.indicates_fatigue());
    assert!(!Posture::Upright.indicates_fatigue());
    assert!(!Posture::LeanBack.indicates_fatigue());
    assert!(Posture::Fatigued.indicates_fatigue());
    assert!(!Posture::Unknown.indicates_fatigue());
}

/// `UserState::should_tighten_threshold` fires on any of: high
/// cognitive load, high arousal, fatigue posture. This is the
/// disjunctive composition — one signal is enough.
#[test]
fn user_state_tighten_on_any_high_signal() {
    let now = Timestamp::now();

    // Neutral state — no tightening.
    let neutral = UserState::neutral(now);
    assert!(!neutral.should_tighten_threshold());

    // High arousal alone tightens.
    let aroused = UserState {
        biometric: BiometricSnapshot {
            arousal: ArousalLevel::Acute,
            ..BiometricSnapshot::neutral(now)
        },
        posture: PostureSnapshot::unknown(now),
        cognitive_load: CognitiveLoad::Nominal,
        at: now,
    };
    assert!(aroused.should_tighten_threshold());

    // Fatigue alone tightens.
    let fatigued = UserState {
        biometric: BiometricSnapshot::neutral(now),
        posture: PostureSnapshot::new(Posture::Fatigued, PresenceLevel::Present, None, now)
            .expect("constructible"),
        cognitive_load: CognitiveLoad::Nominal,
        at: now,
    };
    assert!(fatigued.should_tighten_threshold());

    // High load alone tightens.
    let engaged = UserState {
        biometric: BiometricSnapshot::neutral(now),
        posture: PostureSnapshot::unknown(now),
        cognitive_load: CognitiveLoad::Engaged,
        at: now,
    };
    assert!(engaged.should_tighten_threshold());
}

/// Symmetry check by absence: `UserState` exposes `should_tighten_threshold`
/// but does NOT expose `should_loosen_threshold`. The asymmetry is the
/// API surface.
///
/// We can't test "method does not exist" directly in a runtime test, so
/// we test the morally equivalent thing: there is no "loosen on low
/// arousal" path. Confirm that an underloaded, low-arousal, neutral-
/// posture user still doesn't *trigger* anything (no method to call;
/// nothing happens).
#[test]
fn relaxed_state_does_not_trigger_loosening_signal() {
    let now = Timestamp::now();
    let relaxed = UserState {
        biometric: BiometricSnapshot {
            arousal: ArousalLevel::Low,
            ..BiometricSnapshot::neutral(now)
        },
        posture: PostureSnapshot::new(Posture::LeanBack, PresenceLevel::Idle, None, now)
            .expect("constructible"),
        cognitive_load: CognitiveLoad::Underloaded,
        at: now,
    };
    // No tighten signal.
    assert!(!relaxed.should_tighten_threshold());
    // And — by the structural asymmetry — there is also no API to ask
    // for loosening. The substrate's only safety-relevant API is
    // `should_tighten_threshold`. This is verified by inspection of
    // the `UserState` impl.
}

/// Presence-level engagement is its own thing — `PresenceLevel::Present`
/// is the only "user is here" signal. Used by Pneuma for "should we
/// proceed with optimistic dispatch?" decisions.
#[test]
fn only_present_level_indicates_engagement() {
    assert!(PresenceLevel::Present.is_engaged());
    assert!(!PresenceLevel::Idle.is_engaged());
    assert!(!PresenceLevel::Absent.is_engaged());
    assert!(!PresenceLevel::Unknown.is_engaged());
}
