//! Privacy is a structural property, not a runtime check.
//!
//! Properties under test:
//!
//! - **§7.1** Sensorium "is local-only by hard requirement".
//! - **§4.4 corollary 1** "Passive context dominates" — half the
//!   substrate observes things the user did not actively express, so
//!   the privacy story must be airtight.
//!
//! The compile-time guarantee — `LocalOnly<T>` does not implement
//! `Serialize` — is verified by a `compile_fail` doc-test in
//! `privacy.rs`. This file verifies the *runtime* properties:
//! ordering, composition, redaction round-trip.

use sensorium_core::{LocalOnly, PrivacyTier, Redacted, RedactionReason};

/// `PrivacyTier` ordering is strict: `Sensitive > Private > Public`.
/// Aggregation pipelines depend on this — they take the *strictest*
/// tier of any input via `max`/`strictest`.
#[test]
fn privacy_tier_strictest_takes_max() {
    assert!(PrivacyTier::Sensitive > PrivacyTier::Private);
    assert!(PrivacyTier::Private > PrivacyTier::Public);

    // Composition is idempotent and commutative.
    let pairs = [
        (
            PrivacyTier::Public,
            PrivacyTier::Public,
            PrivacyTier::Public,
        ),
        (
            PrivacyTier::Public,
            PrivacyTier::Private,
            PrivacyTier::Private,
        ),
        (
            PrivacyTier::Private,
            PrivacyTier::Public,
            PrivacyTier::Private,
        ),
        (
            PrivacyTier::Private,
            PrivacyTier::Sensitive,
            PrivacyTier::Sensitive,
        ),
        (
            PrivacyTier::Sensitive,
            PrivacyTier::Public,
            PrivacyTier::Sensitive,
        ),
    ];
    for (a, b, expected) in pairs {
        assert_eq!(a.strictest(b), expected);
        assert_eq!(b.strictest(a), expected, "strictest is commutative");
    }

    // Identity at Public: `tier.strictest(Public) == tier`.
    for tier in [
        PrivacyTier::Public,
        PrivacyTier::Private,
        PrivacyTier::Sensitive,
    ] {
        assert_eq!(tier.strictest(PrivacyTier::Public), tier);
    }
}

/// Permission predicates form a strict descending ladder.
/// Public can do everything, Private can journal but not forward,
/// Sensitive can do nothing without explicit declassification.
#[test]
fn permission_predicates_ladder_strictly() {
    let public = PrivacyTier::Public;
    assert!(public.permits_serialize());
    assert!(public.permits_journal());
    assert!(public.permits_remote_forward());

    let private = PrivacyTier::Private;
    assert!(private.permits_serialize());
    assert!(private.permits_journal());
    assert!(!private.permits_remote_forward());

    let sensitive = PrivacyTier::Sensitive;
    assert!(!sensitive.permits_serialize());
    assert!(!sensitive.permits_journal());
    assert!(!sensitive.permits_remote_forward());
}

/// `LocalOnly::redact` produces a `Redacted<T>` carrying the original
/// tier and the redaction reason. The substrate uses this for
/// journal-with-redaction.
#[test]
fn redact_preserves_tier_and_reason() {
    let raw = LocalOnly::new(42_u32, PrivacyTier::Sensitive);
    let redacted = raw.redact(RedactionReason::PrivacyTier);

    assert_eq!(redacted.tier, PrivacyTier::Sensitive);
    assert_eq!(redacted.reason, RedactionReason::PrivacyTier);
}

/// `LocalOnly::declassify` returns the inner value, dropping the privacy
/// stamp. The caller assumes responsibility from there.
#[test]
fn declassify_returns_inner_value() {
    let raw = LocalOnly::new("secret".to_owned(), PrivacyTier::Sensitive);
    let inner = raw.declassify();
    assert_eq!(inner, "secret");
}

/// `LocalOnly::map` runs inside the privacy boundary, preserving the
/// tier. Local-only computation that consumes the wrapper produces
/// another wrapper.
#[test]
fn map_preserves_privacy_tier() {
    let raw = LocalOnly::new(10_u32, PrivacyTier::Private);
    let mapped: LocalOnly<u64> = raw.map(|n| u64::from(n) * 100);

    assert_eq!(mapped.tier(), PrivacyTier::Private);
    assert_eq!(*mapped.as_inner(), 1000_u64);
}

/// `Redacted<T>` round-trips through serde_json. The wire format is a
/// fixed shape regardless of `T` because `T` itself is *not* serialized.
#[test]
fn redacted_round_trips_through_json() {
    let r: Redacted<String> = Redacted::new(PrivacyTier::Sensitive, RedactionReason::PrivacyTier);
    let json = serde_json::to_string(&r).expect("Redacted is Serialize");
    let de: Redacted<String> = serde_json::from_str(&json).expect("Redacted is Deserialize");
    assert_eq!(de.tier, r.tier);
    assert_eq!(de.reason, r.reason);
}

/// All four redaction reasons survive a round trip — exhaustive coverage
/// so a future variant addition flags here.
#[test]
fn all_redaction_reasons_round_trip() {
    use RedactionReason::{CalibrationFailure, PrivacyTier as TierReason, SizeBudget, TestFixture};

    for reason in [TierReason, CalibrationFailure, SizeBudget, TestFixture] {
        let r: Redacted<u32> = Redacted::new(PrivacyTier::Private, reason);
        let json = serde_json::to_string(&r).expect("redacted encodes");
        let de: Redacted<u32> = serde_json::from_str(&json).expect("redacted decodes");
        assert_eq!(de.reason, reason);
    }
}
