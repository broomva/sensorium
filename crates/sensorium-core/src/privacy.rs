//! Privacy as a structural property.
//!
//! The substrate is **local-only by hard architectural requirement** (see
//! `MIL-PROJECT.md` §7.1). Sensitive observations — gaze fixations,
//! biometric signals, recent activity — must never leave the device unless
//! explicitly declassified.
//!
//! We enforce this with the type system rather than runtime checks:
//!
//! - [`PrivacyTier`] — coarse classification attached to every [`Tagged`]
//!   value via [`crate::provenance::Provenance::privacy`].
//! - [`LocalOnly<T>`] — *typestate* wrapper. `LocalOnly<T>` does not
//!   implement [`serde::Serialize`]; the only way to serialize it is to
//!   [`LocalOnly::redact`] it (returning a [`Redacted<T>`] containing only
//!   metadata) or [`LocalOnly::declassify`] it (returning the inner `T` and
//!   transferring privacy responsibility to the caller).
//!
//! This means a producer that emits a `LocalOnly<GazeFixation>` cannot
//! accidentally serialize it to disk via a generic journaling path — the
//! type system rejects the call.
//!
//! ## What this does *not* protect against
//!
//! - A caller that explicitly declassifies and then misroutes. We can't
//!   prevent that; we make it impossible to do *accidentally*.
//! - Side channels (timing, length). Out of scope.
//! - The substrate writing to local storage. The privacy tier is "local
//!   only", not "memory only".
//!
//! [`Tagged`]: crate::provenance::Tagged

use serde::{Deserialize, Serialize};

// --- PrivacyTier -------------------------------------------------------------

/// Coarse privacy classification.
///
/// Stamped on every [`crate::provenance::Provenance`]. Determines what the
/// substrate may do with the value: journal it (Lago), forward it to a
/// remote agent (Spaces), serialize it for replay (test fixtures).
///
/// The tiers form a strict ordering — `Sensitive > Private > Public`. A
/// pipeline that aggregates values takes the *strictest* tier of any input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PrivacyTier {
    /// Fine to journal, forward, and serialize freely. Workspace structure
    /// (which app is focused, which file is open) typically lives here.
    Public,
    /// Local-only by default. May be journaled to Lago but never forwarded
    /// to remote agents without per-value consent. Recent activity, query
    /// history, file paths typically live here.
    Private,
    /// Never leaves the device. Only declassifiable through an explicit
    /// [`LocalOnly::declassify`] call. Biometric streams, raw gaze, posture
    /// signals live here.
    Sensitive,
}

impl PrivacyTier {
    /// `true` if this tier permits journaling.
    #[must_use]
    pub fn permits_journal(self) -> bool {
        matches!(self, Self::Public | Self::Private)
    }

    /// `true` if this tier permits forwarding to a remote agent.
    #[must_use]
    pub fn permits_remote_forward(self) -> bool {
        matches!(self, Self::Public)
    }

    /// `true` if this tier permits transparent serialization. (Always
    /// `false` for `Sensitive`; the wrapper [`LocalOnly`] enforces this at
    /// the type level.)
    #[must_use]
    pub fn permits_serialize(self) -> bool {
        matches!(self, Self::Public | Self::Private)
    }

    /// Compose two tiers, taking the strictest.
    #[must_use]
    pub fn strictest(self, other: Self) -> Self {
        self.max(other)
    }
}

// --- RedactionReason ---------------------------------------------------------

/// Why a value was redacted.
///
/// Carried in [`Redacted`] so downstream readers know whether the omission
/// was a privacy choice, a calibration failure, or a length-limit truncation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum RedactionReason {
    /// The value was [`LocalOnly`] and the caller chose to redact rather
    /// than declassify.
    PrivacyTier,
    /// The producer's calibration failed and the value is not trustworthy.
    /// We redact to avoid leaking miscalibrated data downstream.
    CalibrationFailure,
    /// The value exceeded an explicit length / size budget. The wire format
    /// keeps the redaction marker so consumers know data was elided.
    SizeBudget,
    /// A test fixture redacted manually to exercise the redaction path.
    TestFixture,
}

// --- Redacted<T> -------------------------------------------------------------

/// A redacted version of `T`: just the metadata.
///
/// This is what serializing a [`LocalOnly<T>`] gets you. The inner value is
/// gone; what remains is the privacy tier (so consumers know what *kind* of
/// thing is missing) and the [`RedactionReason`].
///
/// `Redacted<T>` implements [`Serialize`] / [`Deserialize`]; consumers
/// can round-trip it as a placeholder. `T` itself is *not* serialized —
/// `PhantomData` keeps the type parameter for downstream type-checking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Redacted<T> {
    /// What tier was redacted.
    pub tier: PrivacyTier,
    /// Why the redaction happened.
    pub reason: RedactionReason,
    #[serde(skip)]
    _marker: std::marker::PhantomData<T>,
}

impl<T> Redacted<T> {
    /// Construct a redaction marker.
    #[must_use]
    pub fn new(tier: PrivacyTier, reason: RedactionReason) -> Self {
        Self {
            tier,
            reason,
            _marker: std::marker::PhantomData,
        }
    }
}

// --- LocalOnly<T> ------------------------------------------------------------

/// Typestate wrapper marking a value as never-leaves-the-device by default.
///
/// `LocalOnly<T>` deliberately does **not** implement
/// [`serde::Serialize`]. To serialize:
///
/// - Call [`LocalOnly::redact`] for a `Redacted<T>` placeholder (preferred
///   for journals and traces).
/// - Call [`LocalOnly::declassify`] to take ownership of the inner `T`
///   (caller assumes responsibility for handling).
///
/// This is enforced at compile time. A pipeline that wires `LocalOnly` into
/// `serde_json::to_string` will fail to compile, as the example below
/// asserts.
///
/// ## Example: serialization is a compile error
///
/// ```rust,compile_fail
/// use sensorium_core::{LocalOnly, PrivacyTier};
///
/// let raw = LocalOnly::new(42_u32, PrivacyTier::Sensitive);
/// // Does not compile — `LocalOnly` is not Serialize:
/// let _json = serde_json::to_string(&raw).unwrap();
/// ```
///
/// ## Example: explicit redaction works
///
/// ```rust
/// use sensorium_core::{LocalOnly, PrivacyTier, RedactionReason};
///
/// let raw = LocalOnly::new(42_u32, PrivacyTier::Sensitive);
/// let redacted = raw.redact(RedactionReason::PrivacyTier);
/// let json = serde_json::to_string(&redacted).expect("redaction is serializable");
/// assert!(json.contains("Sensitive"));
/// ```
///
/// ## Why not just check at runtime?
///
/// Because the substrate has hundreds of `Tagged<T>` constructions per
/// second on hot paths, and one missed runtime check is a privacy bug.
/// Compile-time enforcement turns the bug class into a "doesn't compile"
/// class.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LocalOnly<T> {
    inner: T,
    tier: PrivacyTier,
}

impl<T> LocalOnly<T> {
    /// Wrap a value as local-only at the given privacy tier.
    ///
    /// A `Public` tier is allowed but unusual — usually you'd just keep the
    /// `T` directly. The wrapper exists to preserve the privacy stamp
    /// through pipelines that aggregate mixed tiers.
    #[must_use]
    pub fn new(inner: T, tier: PrivacyTier) -> Self {
        Self { inner, tier }
    }

    /// The privacy tier this value carries.
    #[must_use]
    pub fn tier(&self) -> PrivacyTier {
        self.tier
    }

    /// Borrow the inner value.
    ///
    /// **Privacy note**: by design, you can borrow without declassifying so
    /// that local-only computations (computing summaries, aggregating
    /// stats) work normally. The privacy enforcement is around *crossing
    /// boundaries* (serialization, IPC). Don't accidentally pass the
    /// reference into a serializer.
    #[must_use]
    pub fn as_inner(&self) -> &T {
        &self.inner
    }

    /// Take ownership of the inner value, dropping the privacy marker.
    ///
    /// **The caller assumes responsibility for the value's privacy from
    /// here on.** Use sparingly. Prefer [`LocalOnly::redact`] when crossing
    /// a serialization boundary.
    #[must_use]
    pub fn declassify(self) -> T {
        self.inner
    }

    /// Convert to a [`Redacted<T>`] placeholder, dropping the inner value.
    ///
    /// This is the safe way to cross a serialization boundary while
    /// preserving auditability — the redaction marker tells downstream
    /// readers that data was here and was elided.
    #[must_use]
    pub fn redact(self, reason: RedactionReason) -> Redacted<T> {
        Redacted::new(self.tier, reason)
    }

    /// Map the inner value, preserving the privacy tier. The closure runs
    /// inside the privacy boundary.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> LocalOnly<U> {
        LocalOnly {
            inner: f(self.inner),
            tier: self.tier,
        }
    }
}

// --- Note: LocalOnly does NOT implement Serialize/Deserialize. ---------------
// This is the entire point of the type. Compile-time enforcement.

#[cfg(test)]
mod tests {
    use super::*;

    /// Privacy ordering: Sensitive > Private > Public. A monoid-style
    /// composition `strictest` selects the higher tier.
    #[test]
    fn privacy_tier_ordering() {
        assert!(PrivacyTier::Sensitive > PrivacyTier::Private);
        assert!(PrivacyTier::Private > PrivacyTier::Public);
        assert_eq!(
            PrivacyTier::Public.strictest(PrivacyTier::Sensitive),
            PrivacyTier::Sensitive
        );
        assert_eq!(
            PrivacyTier::Private.strictest(PrivacyTier::Public),
            PrivacyTier::Private
        );
    }

    /// Permission predicates ladder correctly: Public can do everything,
    /// Private can journal but not forward, Sensitive can do nothing
    /// without explicit declassification.
    #[test]
    fn privacy_tier_permissions() {
        assert!(PrivacyTier::Public.permits_serialize());
        assert!(PrivacyTier::Public.permits_journal());
        assert!(PrivacyTier::Public.permits_remote_forward());

        assert!(PrivacyTier::Private.permits_serialize());
        assert!(PrivacyTier::Private.permits_journal());
        assert!(!PrivacyTier::Private.permits_remote_forward());

        assert!(!PrivacyTier::Sensitive.permits_serialize());
        assert!(!PrivacyTier::Sensitive.permits_journal());
        assert!(!PrivacyTier::Sensitive.permits_remote_forward());
    }

    /// `LocalOnly::map` preserves the tier. Computing inside the privacy
    /// boundary doesn't escape it.
    #[test]
    fn local_only_map_preserves_tier() {
        let raw = LocalOnly::new(42_u32, PrivacyTier::Sensitive);
        let mapped = raw.map(|n| n + 1);
        assert_eq!(mapped.tier(), PrivacyTier::Sensitive);
        assert_eq!(*mapped.as_inner(), 43);
    }
}
