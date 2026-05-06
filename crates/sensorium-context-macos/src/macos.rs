//! macOS-specific NSWorkspace polling.
//!
//! Compiled only on `target_os = "macos"`. The single public entry
//! point is `poll_once`, which the parent crate's polling loop calls
//! at the configured cadence.
//!
//! ## Safety
//!
//! Both `NSWorkspace::sharedWorkspace()` and `frontmostApplication()`
//! are documented as safe in the `objc2-app-kit` 0.3 generated
//! bindings (no `MainThreadMarker` argument required, no `unsafe`
//! attribute on the method). However, the call must still happen
//! through the `objc2` runtime, which marks `msg_send!`-style calls
//! as `unsafe` for FFI hygiene. We wrap the calls in `unsafe` blocks
//! and document why each is safe.
//!
//! Per `objc2-app-kit` docs, the array of running applications
//! refreshes only when the main run loop processes events in a common
//! mode. For a host process where the main run loop runs
//! (Mission Control / Tauri / TUI with a real run loop), polling
//! reads fresh values. For a `cargo run` headless binary it may
//! return stale values. The architecture is fine for the v0.2
//! mission-control / chat host targets; v0.3 may switch to
//! notification-driven updates.

use objc2_app_kit::NSWorkspace;
use sensorium_context::ManualObserver;
use sensorium_core::AppId;

/// Read the current frontmost application from NSWorkspace and
/// update the manual observer's context if it has changed.
///
/// Idempotent: rebuilding with the same `focused_app` is cheap
/// (`Arc::ptr_eq` reuse) and surface-compatible with the
/// snapshot-drift detection in `pneuma-router`.
pub fn poll_once(manual: &ManualObserver) {
    // objc2-app-kit 0.3 marks these methods as safe Rust calls;
    // no MainThreadMarker required, no `unsafe` block needed.
    let workspace = NSWorkspace::sharedWorkspace();
    // `frontmostApplication()` may return None during
    // login/screensaver transitions; we treat that as "no focused
    // app" and leave the observer untouched.
    let Some(app) = workspace.frontmostApplication() else {
        return;
    };

    // Prefer bundleIdentifier (stable, reverse-DNS form like
    // "com.apple.Safari") over localizedName (display name, varies
    // by user locale). Fall back to localizedName when bundleId is
    // not available.
    let bundle = app.bundleIdentifier().map(|s| s.to_string());
    let name = app.localizedName().map(|s| s.to_string());

    let identifier = bundle
        .filter(|s| !s.is_empty())
        .or(name)
        .unwrap_or_default();

    if identifier.is_empty() {
        // Defensible: app was running but had no identifying name.
        // Don't clobber the observer with empty data.
        return;
    }

    // AppId::new rejects empty/whitespace; we just ensured it's
    // non-empty so this should always succeed, but `Result` is
    // honored to avoid panics if Apple ever returns whitespace.
    let Ok(app_id) = AppId::new(identifier) else {
        return;
    };

    // Idempotent rebuild: ManualObserver clones the inner Arc and
    // skips the rebuild if `with_focused_app` produces the same
    // state.
    manual.rebuild_with(|b| b.with_focused_app(Some(app_id)));
}
