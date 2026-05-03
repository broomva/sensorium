# sensorium-context

Workspace observer providers — the producer layer for the Sensorium
substrate. **Phase 1.1** deliverable of the MIL build.

## What this crate exposes

- [`Observer`] — the trait. `current() -> WorkspaceContext`,
  `snapshot() -> WorkspaceSnapshot`. Pull-based; consumers query when
  they need the substrate.
- [`ManualObserver`] — programmatic. The producer mutates the context
  via `set_focused_file`, `push_activity`, `rebuild_with`. Used in
  tests and scripted demos.
- [`FsObserver`] — filesystem-watching. Uses [`notify`][notify] to
  detect file events under a watched path; on each event, pushes an
  `ActivityMarker` and (optionally) updates `visible_files`. Spawns
  one watcher thread per instance; thread is joined on `Drop`.

[notify]: https://crates.io/crates/notify

## Design rationale

The Sensorium architecture (`MIL-PROJECT.md` §7.1) wants multiple
producers feeding a single shared substrate: workspace observer, gaze
tracker, voice stream, gesture recognizer. This crate ships the
*workspace* producer first because it's the one the demo needs and
it's platform-portable via `notify`.

Pull-based for v0.2 because the demo flow is "build context → snapshot
→ commit → dispatch" (one query per commit). Push/subscribe semantics
are a v0.3 concern when the HUD repaints at 30 Hz.

## Status

v0.2.0 — first public producer for the Sensorium substrate. The
macOS-specific observer using NSWorkspace + accessibility tree is
deferred to a future `sensorium-context-macos` crate.

See [MIL-PROJECT.md](../../../../MIL-PROJECT.md) §11.2 for Phase 1.
