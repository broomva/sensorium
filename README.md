# Sensorium — sensory apparatus for the Life Agent OS

Multimodal input substrate for [MIL — the Multimodal Intent Language](../../MIL-PROJECT.md).
Sensorium turns world observations into typed `PrimitiveToken` streams and a queryable
`WorkspaceContext`. It is exteroception (the world → the agent), the dual of Vigil
(proprioception, the agent → itself).

## Crates

- [`sensorium-core`](crates/sensorium-core) — types-only foundation: workspace context,
  primitive-token taxonomy, sensor metadata, privacy markers. No I/O.

Future phase-1+ crates (not yet built): `sensorium-context` (workspace observers),
`sensorium-vision` (MediaPipe/WiLoR), `sensorium-voice` (Whisper), `sensorium-gaze`
(eye tracking), `sensorium-headset` (Quest 3 / Vision Pro), `sensorium-lago` (replay
journal bridge).

## Status

v0.2.0 — types-only, phase 0 of the MIL build order. See `MIL-PROJECT.md` §11.
