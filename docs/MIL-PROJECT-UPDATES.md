# MIL-PROJECT.md proposed updates — sensorium-core v0.2.0

This file holds the proposed deltas to your master `MIL-PROJECT.md` after
the `sensorium-core` crate landed. Two sections are affected: §11 (build
order) and §13 (decisions log). Everything else is unchanged.

The crate was built at `~/broomva/core/sensorium/crates/sensorium-core/`,
mirroring the prosopon workspace convention.

---

## Replace §10 with the addition of a §10.5 — what `sensorium-core` ships

Append this subsection at the end of §10 (alongside §10.1–§10.4 about
`pneuma-core`) so both sibling crates are documented in lockstep.

### 10.5 The `sensorium-core` crate

Status: **compiles, 79 unit + integration tests pass, 1 compile_fail doctest
pass, clippy clean (`-D warnings`).**

Lives at `~/broomva/core/sensorium/crates/sensorium-core/`.

**Module layout**

| Module          | What's in it                                          | Key types                                                                                                |
| --------------- | ----------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| `time.rs`       | Wall-clock + monotonic time                           | `Timestamp`, `Monotonic`                                                                                 |
| `entity.rs`     | Workspace entity types                                | `AppId`, `WindowId`, `WindowRect`, `FileRef`, `MimeType`, `SymbolRef`, `SelectionRef`, `TextSpan`, `Uri` |
| `sensor.rs`     | Producer identity + calibration                       | `SensorId`, `SensorKind`, `SensorMetadata`, `Calibration`, `CalibrationStatus`                           |
| `privacy.rs`    | Privacy as a structural property                      | `PrivacyTier`, `LocalOnly<T>`, `Redacted<T>`, `RedactionReason`                                          |
| `provenance.rs` | Universal `Tagged<T>` wrapper                         | `Tagged<T>`, `Provenance`                                                                                |
| `primitive.rs`  | The seven-primitive taxonomy                          | `PrimitiveKind` (closed enum, 7 variants, `ALL` const)                                                   |
| `ring.rs`       | Bounded const-generic ring buffer                     | `RingBuffer<T, N>`, `RingIter`, `RingIterRev`                                                            |
| `attention.rs`  | Gaze types                                            | `GazePoint`, `GazeSample`, `Fixation`, `GazeFixation`                                                    |
| `biometric.rs`  | Biometric snapshot                                    | `HeartRate`, `SkinConductance`, `ArousalLevel`, `BiometricSnapshot`                                      |
| `posture.rs`    | Posture / presence                                    | `Posture`, `PresenceLevel`, `PostureSnapshot`                                                            |
| `state.rs`      | Aggregated user state                                 | `CognitiveLoad`, `UserState` (read by Autonomic for threshold adjustment)                                |
| `token.rs`      | Seven-primitive event taxonomy                        | `PrimitiveToken` (7 variants), `ApprovalEvent`, `RelationEvent`, `ModulationEvent`, `AttentionEvent`     |
| `workspace.rs`  | The substrate keystone                                | `WorkspaceContext` (Arc-backed), `WorkspaceState`, `WorkspaceSnapshot`, `WorkspaceSnapshotId`, `WorkspaceContextBuilder`, `RecentActivity`, `ActivityMarker` |
| `error.rs`      | Substrate-construction errors                         | `SensoriumError`, `Result<T, SensoriumError>`                                                            |
| `lib.rs`        | Re-exports + `GRAMMAR_VERSION = "0.2.0"`              | —                                                                                                        |

**Key design decisions in the crate**

- **`Arc`-backed `WorkspaceContext`.** Cloning the context bumps a refcount.
  Snapshot capture is `Arc::clone(&state)` plus a fresh `WorkspaceSnapshotId`
  — `O(1)` regardless of state size. This is the answer to "cheap to take
  snapshots, they happen at every commit" from the brief.
- **Two-axis snapshot identity.** `WorkspaceSnapshotId` (UUIDv7) answers
  "is this the same record?"; `WorkspaceSnapshot::observes_same_state`
  (Arc::ptr_eq) answers "did the world change?". Conflating them is the
  bug class the snapshot machinery exists to prevent. Both axes are
  tested independently (`tests/snapshot_identity.rs`).
- **Privacy as a typestate.** `LocalOnly<T>` does not implement
  `Serialize`. Crossing a serialization boundary requires either
  `LocalOnly::redact` (returns `Redacted<T>` placeholder, journal-friendly)
  or `LocalOnly::declassify` (caller assumes responsibility). A
  `compile_fail` doctest in `privacy.rs` verifies the compile-time
  enforcement directly.
- **`PrimitiveKind` is a closed enum, not `#[non_exhaustive]`.** The
  seven-primitive partition is load-bearing. Adding an eighth must break
  downstream pattern matches by design — that forces a design discussion,
  not an enum extension.
- **Custom serde for `RingBuffer`.** Serde does not provide a generic
  `[T; N]: Serialize` for arbitrary const `N`. The ring serializes as a
  flat sequence of `T` in oldest-first order; deserialization rebuilds
  the ring via `push`. This means a sequence longer than `N` evicts the
  leading items, matching push semantics.
- **`UserState::should_tighten_threshold` exists; no symmetric loosening
  API exists.** The asymmetry from §10.2 ("urgency does not lower the
  confidence threshold; only shortens ratify dwell") is encoded as the
  *absence of an API*. A future maintainer cannot accidentally introduce
  loosening; doing so would require writing the method.
- **Calibration is honest, not optional.** `Provenance` has five required
  fields (sensor, observed_at, calibration, privacy, primitive). Producers
  that don't know a field must lie deliberately — there is no "field not
  provided" path. The 20% confidence penalty for uncalibrated values
  lives in `pneuma-core`, not here; the substrate's job is honest
  reporting.

**Tests covering**

- Snapshot identity: ID monotonicity, Arc structural equality, drift
  detection via rebuild, builder isolation.
- Calibration propagation: trusted-only-for-`Calibrated`, conservative
  aggregation, `Tagged::map` provenance preservation.
- Primitive taxonomy: exactly 7 variants, exactly 3 passive, exactly 1
  language-model-required, exactly 1 binary-safety-critical.
- Privacy markers: tier ordering and strictest composition, permission
  predicates, redaction round-trip, `compile_fail` for `LocalOnly`
  serialization.
- Safety asymmetry: every "tighten" trigger fires; relaxed state cannot
  loosen because there is no API.
- Serialization: every public type round-trips through serde_json,
  including all 7 `PrimitiveToken` variants.
- Entity validation: empty IDs rejected at construction, span ordering
  enforced, MIME type case-normalized, window rect hit-test correct.
- Workspace substrate: clone shares Arc, builder rebuilds break
  structural equality, ring capacity bounded at 32 even after 200
  pushes.

**Cargo.toml**

```toml
[package]
name = "sensorium-core"
version = "0.2.0"
edition = "2024"
rust-version = "1.85"

[dependencies]
serde = { version = "1.0", features = ["derive", "rc"] }
serde_json = "1.0"
uuid = { version = "1.10", features = ["v7", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
thiserror = "1.0"
semver = { version = "1.0", features = ["serde"] }

[dev-dependencies]
serde_test = "1.0"
```

---

## Replace §11.1 (Phase 0: contracts and dispatch)

```diff
-### 11.1 Phase 0: contracts and dispatch (in progress)
-
-1. ✅ Define directive type system (`pneuma-core`)
-1. ⏳ Define context substrate types (`sensorium-core`) — **next**
-1. ⏳ Define agent response types (already in `pneuma-core::response`; may extract to `prosopon-core`)
-1. ⏳ Build the router as a pure function (`Directive<Committed> + WorkspaceContext → Dispatch`)
-1. ⏳ Build a Praxis adapter for one platform (deterministic dispatch)
-1. ⏳ Build a minimal Arcan adapter (agent dispatch with constrained decoding)
+### 11.1 Phase 0: contracts and dispatch (in progress)
+
+1. ✅ Define directive type system (`pneuma-core`)
+1. ✅ Define context substrate types (`sensorium-core`) — landed 2026-05-02
+1. ⏳ Define agent response types (already in `pneuma-core::response`; may extract to `prosopon-core`) — **next**
+1. ⏳ Build the router as a pure function (`Directive<Committed> + WorkspaceContext → Dispatch`)
+1. ⏳ Build a Praxis adapter for one platform (deterministic dispatch)
+1. ⏳ Build a minimal Arcan adapter (agent dispatch with constrained decoding)
```

The router (item 4) is now genuinely buildable as a pure function: it
needs `Directive<Committed>` from `pneuma-core` and `WorkspaceContext`
from `sensorium-core`, both of which are types-only crates with no I/O.

---

## Append to §13 (decisions log)

Add these decision records to the bottom of §13.

**Sensorium and Pneuma do not depend on each other (yet).** Both define
structurally-equivalent ID types (`AppId`/`WindowId`/`FileRef`/etc.;
`WorkspaceSnapshotId` ↔ `ContextSnapshotId`). The natural canonical home
for entity types is sensorium-core (these are sensor outputs), but
moving them out of pneuma-core would have been a breaking change to a
landed v0.2.0 crate. Decision: accept the duplication for v0.2; build a
`pneuma-sensorium` glue crate later that provides `From`/`Into` impls;
in v0.3 collapse the duplication if it proves a problem. The wire
formats are byte-identical so journals won't need migration.

**`WorkspaceContext` is `Arc`-backed, not `Cow`-backed.** Considered
`Cow<'_, WorkspaceState>` so producers could expose either an owned or
borrowed substrate. Rejected because Pneuma's parser holds the substrate
across multiple call frames during composition; lifetime-parameterized
state would force `'a` parameters through every parser type. `Arc` is
the cleaner fit: cheap clone, no lifetimes, supports cross-thread sharing
when we later move parsing onto a worker. The cost is one `Arc::new`
per substrate update (one allocation per producer tick); acceptable.

**Snapshot identity is two-axis on purpose.** Considered using a
content-addressed cryptographic hash so two snapshots of identical state
have identical IDs. Rejected: hashing the entire `WorkspaceState` on
every commit is expensive (the visible-files list, sensor metadata, etc.),
and the architecture *wants* every capture to have a fresh ID for
journal ordering anyway. The "did the world change?" question is
answered by `Arc::ptr_eq`, which is `O(1)` and free of hashing concerns.
Two distinct dimensions, two distinct APIs.

**`PrimitiveKind` is closed (no `#[non_exhaustive]`).** Considered
marking the enum non-exhaustive so a future eighth primitive could be
added without breaking downstream code. Rejected: the seven-primitive
partition is the architecture's central claim. Hiding extension behind
`#[non_exhaustive]` would let the architecture quietly expand without
review; we want a future eighth to break compilation everywhere it's
matched. Adding `Custom(u32)` to `RelationEvent` *was* approved — that's
a within-primitive vocabulary expansion, not an architectural shift.

**Privacy is a typestate, not a runtime check.** Considered a single
`PrivacyTier` enum field plus runtime guards in serializers. Rejected:
the substrate has hot paths with hundreds of `Tagged<T>` constructions
per second; a missed runtime check is a privacy bug. `LocalOnly<T>` not
implementing `Serialize` turns the bug class into a doesn't-compile
class. Verified directly by a `compile_fail` doctest.

**Calibration is required on every `Provenance`, not optional.**
Considered `Option<Calibration>` so producers without calibration
machinery could pass through. Rejected: the architecture's confidence
story rests on calibration being honest. Producers that don't know must
emit `Calibration::synthetic()` or `Calibration::uncalibrated()`
explicitly — *the substrate refuses to lie on their behalf*. This forces
the calibration story to surface in the producer's own code, where it
belongs.

**`UserState` exposes `should_tighten_threshold` and nothing else.** The
safety-asymmetry property from §10.2 ("urgency does not lower thresholds")
is enforced by the *absence of a method*. Considered a symmetric
`should_loosen_threshold` for completeness. Rejected: completeness here
is the wrong objective. The asymmetric API surface is the architectural
guarantee. A test (`safety_asymmetry::relaxed_state_does_not_trigger_loosening_signal`)
documents the absence so future maintainers don't introduce one
accidentally.

---

## End of proposed updates
