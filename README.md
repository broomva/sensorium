# Sensorium — sensory apparatus for the Life Agent OS

Multimodal input substrate for [MIL — the Multimodal Intent Language](../../MIL-PROJECT.md).
Sensorium turns world observations into typed `PrimitiveToken` streams and a queryable
`WorkspaceContext`. It is exteroception (the world → the agent), the dual of Vigil
(proprioception, the agent → itself).

## Crates

| Crate | Role | Tests |
|---|---|---|
| [`sensorium-core`](crates/sensorium-core) | Substrate types — `WorkspaceContext`, `WorkspaceSnapshot`, `PrimitiveToken`, `Tagged<T>`, privacy markers, ring buffer, calibrated provenance. No I/O. | 81 |
| [`sensorium-context`](crates/sensorium-context) | Pull-based `Observer` trait + `ManualObserver` (tests/demos) + `FsObserver` (notify-based, real-world tokens). Phase 1.1. | 13 |
| [`sensorium-context-macos`](crates/sensorium-context-macos) | macOS workspace observer — polls `NSWorkspace` for the frontmost application. Step #15. | 4 + 1 ignored |
| [`sensorium-voice`](crates/sensorium-voice) | Voice-input substrate — `SpeechToText` trait + `MockStt` + `VoiceSession`. Step #17. Real Parakeet TDT (EOU streaming) inference lands in a follow-up behind `feature = "parakeet"`. | 9 |

**Total tests:** 107 · 2 ignored (interactive) · all green on `cargo test --workspace`.

Future Phase-1.2+ crates (not yet built): `sensorium-context-macos` (NSWorkspace +
Accessibility API), `sensorium-vision` (MediaPipe / WiLoR), `sensorium-voice`
(Whisper streaming), `sensorium-gaze` (eye tracking), `sensorium-headset`
(Quest 3 / Vision Pro), `sensorium-lago` (replay journal bridge).

## Status

v0.2.0 — Phase 1.1 of the MIL build order shipped.
The `Observer` trait is the boundary between Sensorium (which produces world
observations) and Pneuma (which consumes them as `WorkspaceContext`).
`FsObserver` is the first observer that produces real-world tokens; downstream
the `pneuma-demo` binary takes `Box<dyn Observer>` so callers can swap
`ManualObserver` (tests) for `FsObserver` (the rename demo) at runtime.

## What's wired up today

`broomva/pneuma`'s demo binary depends on `sensorium-context` via a path
dependency (`../sensorium`). That demo runs the full pipeline end-to-end on
a tempdir: an `FsObserver` watches the tempdir, a directive parses out of an
`MIL_UTTERANCE` env var, the router dispatches to the Praxis bridge, the
journal records the run, and the HUD prints frames. See
[`MIL-PROJECT.md`](../../MIL-PROJECT.md) §10 for the full crate-by-crate
breakdown and [`docs/mil/progress-snapshot-tier-2-complete.md`](../../docs/mil/progress-snapshot-tier-2-complete.md)
for the current state across both repos.

## Cross-references

- [`MIL-PROJECT.md`](../../MIL-PROJECT.md) §10.5–§10.6 — sensorium-core and sensorium-context build notes
- [`MIL-PROJECT.md`](../../MIL-PROJECT.md) §13 — design decisions (D-2026-05-02-01..07, D-2026-05-03-15..16 are sensorium-related)
- [`docs/mil/router-and-harness.md`](../../docs/mil/router-and-harness.md) — where Sensorium sits in the MIL stack
