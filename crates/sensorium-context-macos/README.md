# sensorium-context-macos — macOS workspace observer

The first **real-world workspace observer** for the Sensorium substrate.
Polls `NSWorkspace.shared.frontmostApplication` once per second on a
background thread and updates a shared `WorkspaceContext` so consumers
(notably `pneuma-resolver`) can read which app is currently focused.

Step #15 of `MIL-PROJECT.md` §11.2. Sibling of `FsObserver` —
same `Observer` trait, same background-thread + Drop-join pattern,
different signal source.

## What's tracked in v0.2

- `focused_app: AppId` — populated from `frontmostApplication.bundleIdentifier`
  if available, falling back to `localizedName`.

## What's NOT tracked in v0.2

- `focused_window: WindowId` — requires the macOS Accessibility API
  (`AXUIElement` + `kAXFocusedWindowAttribute`) and **user-granted
  Accessibility permission**. Researched in `superwhisper-voice-ecosystem`
  research notes; deferred to v0.3 along with the runtime-permission
  prompt flow.
- Window bounds, hidden/visible state, minimization — also AX-gated.
- Notification-driven updates (`NSWorkspaceDidActivateApplicationNotification`).
  v0.2 polls at 1Hz; v0.3 will subscribe to AppKit notifications for
  zero-latency updates.

## Cross-platform behavior

On `target_os != "macos"`, the crate compiles as a stub: the
`MacOsWorkspaceObserver` constructor returns `Ok(...)` but the
background thread is never spawned and `current()` always returns
the initial empty `WorkspaceContext`. This keeps the dependent
crates (notably `pneuma-resolver`) cross-platform-compilable.

## Dependencies

On macOS:
- `objc2` 0.6 — modern Objective-C runtime bindings (canonical 2026 choice)
- `objc2-foundation` — NSString / NSArray
- `objc2-app-kit` — NSWorkspace / NSRunningApplication

No dependencies on non-macOS platforms.

## Status

v0.2.0 — frontmost-application polling at 1Hz. Tests:
cross-platform stub coverage + macOS-gated `#[ignore]`'d live
integration test.
