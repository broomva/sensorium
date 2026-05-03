# sensorium-core

Types-only foundation for the Sensorium subsystem of the Life Agent OS.

`sensorium-core` is the substrate the rest of MIL queries on every parse. It
defines:

- **`WorkspaceContext`** — the queryable substrate shared between user and agent.
  Backed by `Arc<WorkspaceState>` so that taking a snapshot is `O(1)` regardless
  of state size.
- **`WorkspaceSnapshot`** — content-addressed point-in-time projection. Snapshot
  identity is derived from a stable digest of the substrate so two snapshots of
  the same world are byte-identical.
- **`PrimitiveToken`** — the seven-primitive event taxonomy (reference,
  predication, modulation, relation, approval, attention, state). The lexicon
  shared between Sensorium producers and Pneuma consumers.
- **`Tagged<T>`** — every observed value carries `SensorId`, `Calibration`, and
  `Provenance`. Nothing in the substrate is bare.
- **`PrivacyTier` / `LocalOnly`** — substrate is local-only by hard requirement.
  Privacy is a structural property, not a runtime check.

It performs no I/O, holds no mutable state, runs no observers. Producers
(`sensorium-vision`, `sensorium-voice`, `sensorium-gaze`, `sensorium-context`)
are separate crates that depend on this one.

## Status

v0.2.0. Types-only. Used by `pneuma-router` and downstream parsers.

See [MIL-PROJECT.md](../../../MIL-PROJECT.md) §11 for the full build order.
