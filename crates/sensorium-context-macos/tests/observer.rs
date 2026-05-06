//! Integration tests for [`MacOsWorkspaceObserver`].
//!
//! Properties under test:
//!
//! 1. **Constructor never blocks** — even on macOS, `start_default()`
//!    returns immediately; the polling thread runs async.
//! 2. **Initial context is empty** — observer's first `current()` has
//!    no focused_app (the polling thread hasn't run yet).
//! 3. **Drop joins the thread** — explicit drop and stop() are
//!    idempotent and don't deadlock.
//! 4. **Cross-platform compile** — on non-macOS, the observer is a
//!    stub but compiles and constructs cleanly.
//! 5. **(macOS only, gated)** Live frontmost-app detection — after
//!    a poll cycle, `current().focused_app` is populated. Requires
//!    a real GUI session, so `#[ignore]`'d.

use std::time::Duration;

use sensorium_context::Observer;
use sensorium_context_macos::MacOsWorkspaceObserver;

// --- Property 1: Constructor doesn't block --------------------------------

#[test]
fn start_default_returns_immediately() {
    let start = std::time::Instant::now();
    let _obs = MacOsWorkspaceObserver::start_default().expect("constructor must succeed");
    let elapsed = start.elapsed();
    // Generous: 50ms is plenty even on slow CI macOS runners. The
    // constructor only spawns a thread, no I/O.
    assert!(
        elapsed < Duration::from_millis(50),
        "constructor should not block on the first poll; took {elapsed:?}"
    );
}

// --- Property 2: Initial context is empty ----------------------------------

#[test]
fn initial_context_has_no_focused_app() {
    // `current()` runs immediately after construction, before the
    // first poll. focused_app should be None.
    let obs = MacOsWorkspaceObserver::start(Duration::from_secs(60)).expect("start");
    let ctx = obs.current();
    assert!(
        ctx.focused_app().is_none(),
        "before first poll, focused_app must be None; got {:?}",
        ctx.focused_app()
    );
}

// --- Property 3: Drop is idempotent ----------------------------------------

#[test]
fn drop_joins_thread_cleanly() {
    {
        let obs = MacOsWorkspaceObserver::start_default().expect("start");
        let _ctx = obs.current();
        // Drop fires here.
    }
    // If we reach this line, drop didn't deadlock. That's the
    // assertion.
}

#[test]
fn explicit_stop_is_idempotent() {
    let obs = MacOsWorkspaceObserver::start_default().expect("start");
    obs.stop();
    obs.stop(); // Second stop must be a no-op, not a panic / deadlock.
    obs.stop();
}

// --- Property 4: Cross-platform stub --------------------------------------

#[cfg(not(target_os = "macos"))]
#[test]
fn non_macos_stub_constructs_and_returns_empty_context() {
    let obs = MacOsWorkspaceObserver::start_default().expect("stub start");
    // Wait long enough that a real polling thread would have run.
    std::thread::sleep(Duration::from_millis(100));
    let ctx = obs.current();
    assert!(
        ctx.focused_app().is_none(),
        "stub observer should never populate focused_app"
    );
}

// --- Property 5: Live macOS detection (gated) ------------------------------

/// Real end-to-end test against macOS. Requires a GUI session
/// (frontmost application is undefined on headless macOS CI runners).
/// Disabled by default; run manually with:
///
/// ```bash
/// cargo test -p sensorium-context-macos --test observer -- --ignored
/// ```
#[cfg(target_os = "macos")]
#[test]
#[ignore = "requires GUI session — run manually"]
fn macos_live_frontmost_app_is_populated() {
    let obs = MacOsWorkspaceObserver::start(Duration::from_millis(100)).expect("start");
    // Wait for at least one poll cycle.
    std::thread::sleep(Duration::from_millis(300));
    let ctx = obs.current();
    let focused = ctx.focused_app();
    assert!(
        focused.is_some(),
        "expected a frontmost app on a real GUI session; got None"
    );
    // Don't assert specific app — depends on user's environment.
    // Just print for debugging.
    eprintln!("focused_app: {focused:?}");
}
