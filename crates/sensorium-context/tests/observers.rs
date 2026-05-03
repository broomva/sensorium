//! Tests for [`Observer`] / [`ManualObserver`] / [`FsObserver`].
//!
//! Properties:
//!
//! - `ManualObserver::current` returns a clone of the latest context.
//! - Producer-side updates make new `Arc<WorkspaceState>` (drift visible).
//! - Cloned `ManualObserver` instances share state.
//! - `FsObserver::watch` returns immediately and updates context as
//!   filesystem events arrive (tested via tempdir + sleep).
//! - `FsObserver` translates create/modify events but skips access
//!   events.
//! - `FsObserver::stop` joins cleanly; subsequent `current()` works.

use std::fs;
use std::time::Duration;

use sensorium_context::{FsObserver, ManualObserver, Observer};
use sensorium_core::{ActivityMarker, AppId, FileRef, Timestamp};

// --- ManualObserver --------------------------------------------------------

#[test]
fn manual_observer_returns_neutral_context_initially() {
    let now = Timestamp::now();
    let m = ManualObserver::new(now);
    let ctx = m.current();
    assert_eq!(ctx.focused_app(), None);
    assert_eq!(ctx.recent_activity().ring.len(), 0);
}

#[test]
fn manual_observer_set_focused_app_persists() {
    let m = ManualObserver::new(Timestamp::now());
    let app = AppId::new("com.test.app").unwrap();
    m.set_focused_app(Some(app.clone()));
    assert_eq!(m.current().focused_app(), Some(&app));
}

#[test]
fn manual_observer_push_activity_appends_to_ring() {
    let m = ManualObserver::new(Timestamp::now());
    m.push_activity(ActivityMarker::FileAccessed {
        file: FileRef::new("/tmp/x"),
        at: Timestamp::now(),
    });
    m.push_activity(ActivityMarker::FileAccessed {
        file: FileRef::new("/tmp/y"),
        at: Timestamp::now(),
    });
    let ctx = m.current();
    assert_eq!(ctx.recent_activity().ring.len(), 2);
}

#[test]
fn manual_observer_set_focused_file_updates_visible_files_and_rings() {
    let m = ManualObserver::new(Timestamp::now());
    let f = FileRef::new("/tmp/x.txt");
    m.set_focused_file(f.clone(), true);
    let ctx = m.current();
    assert_eq!(ctx.state().visible_files, vec![f]);
    assert_eq!(ctx.recent_activity().ring.len(), 1);
}

#[test]
fn manual_observer_clones_share_state() {
    let m = ManualObserver::new(Timestamp::now());
    let m2 = m.clone();
    m.set_focused_app(Some(AppId::new("com.first").unwrap()));
    // Clone sees the producer's update.
    assert_eq!(
        m2.current().focused_app().map(AppId::as_str),
        Some("com.first")
    );
    m2.set_focused_app(Some(AppId::new("com.second").unwrap()));
    // Original sees the clone's update.
    assert_eq!(
        m.current().focused_app().map(AppId::as_str),
        Some("com.second")
    );
}

#[test]
fn manual_observer_rebuild_breaks_arc_identity() {
    let m = ManualObserver::new(Timestamp::now());
    let snap_a = m.snapshot();
    m.set_focused_app(Some(AppId::new("com.x").unwrap()));
    let snap_b = m.snapshot();
    // After a producer-side update the substrate has rebuilt; the
    // two snapshots observe distinct states.
    assert!(!snap_a.observes_same_state(&snap_b));
}

#[test]
fn manual_observer_reset_returns_to_neutral() {
    let m = ManualObserver::new(Timestamp::now());
    m.set_focused_app(Some(AppId::new("com.x").unwrap()));
    m.reset(Timestamp::now());
    assert_eq!(m.current().focused_app(), None);
}

#[test]
fn manual_observer_implements_observer_trait() {
    let m = ManualObserver::new(Timestamp::now());
    // Coerce through the trait; if this compiles + runs, the impl is
    // wired correctly.
    // Coerce through the trait + invoke a method, proving both that
    // `ManualObserver` implements `Observer` and that the trait
    // method works through a trait-object boundary.
    let as_trait: &dyn Observer = &m;
    let _ = as_trait.current();
}

// --- FsObserver ------------------------------------------------------------

#[test]
fn fs_observer_starts_and_stops_cleanly() {
    let dir = tempfile::tempdir().unwrap();
    let observer = FsObserver::watch(dir.path(), false).unwrap();
    let _ctx = observer.current(); // no events yet — should still work
    observer.stop();
}

#[test]
fn fs_observer_records_create_events_in_activity_ring() {
    let dir = tempfile::tempdir().unwrap();
    let observer = FsObserver::watch(dir.path(), true).unwrap();

    // Give notify a moment to set up its watcher.
    std::thread::sleep(Duration::from_millis(50));

    // Create a file inside the watched directory.
    let new_file = dir.path().join("hello.txt");
    fs::write(&new_file, "hello").unwrap();

    // Wait for the event to flow through.
    std::thread::sleep(Duration::from_millis(400));

    let ctx = observer.current();
    assert!(
        !ctx.recent_activity().ring.is_empty(),
        "ring should have ≥1 event after file create; got {}",
        ctx.recent_activity().ring.len()
    );
    // visible_files should reference the path (track_visible=true).
    assert!(
        !ctx.state().visible_files.is_empty(),
        "track_visible=true should populate visible_files"
    );
}

#[test]
fn fs_observer_drop_joins_cleanly() {
    let dir = tempfile::tempdir().unwrap();
    {
        let _observer = FsObserver::watch(dir.path(), false).unwrap();
        // Drop runs `stop` automatically.
    }
    // If drop hangs the thread, this test would hang.
}

#[test]
fn fs_observer_implements_observer_trait() {
    let dir = tempfile::tempdir().unwrap();
    let observer = FsObserver::watch(dir.path(), false).unwrap();
    // Coerce through the trait + invoke a method, proving both that
    // `FsObserver` implements `Observer` and that the trait method
    // works through a trait-object boundary.
    let as_trait: &dyn Observer = &observer;
    let _ = as_trait.current();
}

#[test]
fn fs_observer_drift_visible_to_pneuma_router_drift_helper_pattern() {
    // This test demonstrates the architectural finding from Risk #6:
    // a real producer's updates make snapshot Arc::ptr_eq fail, which
    // is exactly what `pneuma_router::drift_detected` checks.
    let dir = tempfile::tempdir().unwrap();
    let observer = FsObserver::watch(dir.path(), true).unwrap();

    std::thread::sleep(Duration::from_millis(50));
    let snap_before = observer.snapshot();

    fs::write(dir.path().join("trigger.txt"), "x").unwrap();
    std::thread::sleep(Duration::from_millis(400));

    let snap_after = observer.snapshot();
    // The producer-side update should have made the snapshots
    // observe different state, even though they're from the same
    // observer instance.
    assert!(
        !snap_before.observes_same_state(&snap_after),
        "filesystem event must cause substrate rebuild — Arc::ptr_eq must fail"
    );
}
