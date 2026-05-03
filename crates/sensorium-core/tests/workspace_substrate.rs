//! WorkspaceContext is cheap to query and cheap to snapshot.
//!
//! Properties under test (cross-references to `MIL-PROJECT.md`):
//!
//! - **§5.2** "Continuously projected workspace state" — Pneuma reads
//!   it on every parse. Allocations on every parse would be a
//!   performance disaster.
//! - **§7.1** Sensorium subsystem layout — `sensorium-context` is the
//!   workspace observer; `sensorium-core` is the substrate type.
//! - **The brief**: "how to make the substrate cheap to query (it's
//!   read on every parse), how to make snapshots cheap to take (they
//!   happen at every commit)".
//!
//! What "cheap" means structurally: clones share the inner `Arc`;
//! snapshot capture is `Arc::clone` of the state; queries borrow
//! through the `Arc` without allocation. We can't measure performance
//! in a unit test, but we can verify the *structural* property: the
//! same `Arc` flows through.

use std::sync::Arc;

use sensorium_core::{
    ActivityMarker, AppId, FileRef, SelectionRef, SensorKind, SensorMetadata, SymbolRef, TextSpan,
    Timestamp, WindowId, WorkspaceContext, WorkspaceContextBuilder, WorkspaceState,
};

/// Cloning a context shares the inner `Arc`. Constant-time, no
/// allocation. (Other than the Arc bookkeeping.)
#[test]
fn cloning_context_shares_arc() {
    let ctx = WorkspaceContext::neutral(Timestamp::now());
    let cloned = ctx.clone();
    assert!(ctx.shares_state_with(&cloned));
}

/// Snapshot capture from a cloned context shares the same `Arc` as a
/// snapshot from the original. The whole point of `Arc`-backed context.
#[test]
fn snapshot_from_clone_shares_arc_with_snapshot_from_original() {
    let ctx = WorkspaceContext::neutral(Timestamp::now());
    let cloned = ctx.clone();

    let s1 = ctx.snapshot();
    let s2 = cloned.snapshot();

    assert!(Arc::ptr_eq(&s1.state, &s2.state));
}

/// The builder, given `from_context`, deep-clones the inner state. This
/// is the *correct* behavior because mutating after build must not
/// affect the originating context. The cost is one allocation per
/// build; this matches the producer pattern (one build per observer
/// tick).
#[test]
fn builder_from_context_does_not_share_state_after_build() {
    let ctx_a = WorkspaceContext::neutral(Timestamp::now());
    let ctx_b = WorkspaceContextBuilder::from_context(&ctx_a)
        .with_focused_app(Some(AppId::new("com.b.app").unwrap()))
        .build();

    assert!(!ctx_a.shares_state_with(&ctx_b));
    assert_eq!(ctx_a.focused_app(), None);
    assert_eq!(ctx_b.focused_app().map(AppId::as_str), Some("com.b.app"));
}

/// The builder mints a fresh state — the previous context's snapshots
/// are unaffected. This is what makes the substrate's copy-on-write
/// model safe across long-lived snapshot references.
#[test]
fn builder_does_not_invalidate_existing_snapshots() {
    let ctx_a = WorkspaceContext::neutral(Timestamp::now());
    let snapshot_before_rebuild = ctx_a.snapshot();

    let _ctx_b = WorkspaceContextBuilder::from_context(&ctx_a)
        .with_focused_app(Some(AppId::new("com.b.app").unwrap()))
        .build();

    // snapshot_before_rebuild still points at the original state.
    assert_eq!(snapshot_before_rebuild.state.focused_app, None);
}

/// Building a context with all the workspace fields populated and
/// reading them back works. Smoke test on the builder surface.
#[test]
fn builder_populates_all_fields() {
    let now = Timestamp::now();
    let app = AppId::new("com.test.app").unwrap();
    let win = WindowId::new("win-1").unwrap();
    let file = FileRef::new("/tmp/x.rs");
    let symbol = SymbolRef::new(file.clone(), "module::func").unwrap();
    let span = TextSpan::new(0, 10).unwrap();
    let sel = SelectionRef::new(file.clone(), span);
    let sensor_meta = SensorMetadata::new(SensorKind::Workspace);

    let ctx = WorkspaceContextBuilder::neutral(now)
        .with_focused_app(Some(app.clone()))
        .with_focused_window(Some(win.clone()))
        .with_visible_files(vec![file.clone()])
        .with_selection(Some(sel.clone()))
        .with_sensor(sensor_meta.clone())
        .push_activity(ActivityMarker::FileAccessed {
            file: file.clone(),
            at: now,
        })
        .build();

    assert_eq!(ctx.focused_app(), Some(&app));
    assert_eq!(ctx.focused_window(), Some(&win));
    assert_eq!(ctx.selection(), Some(&sel));
    assert_eq!(ctx.state().visible_files, vec![file.clone()]);
    assert_eq!(ctx.state().sensors, vec![sensor_meta]);
    assert_eq!(ctx.recent_activity().ring.len(), 1);

    // Use the symbol for coverage of the constructor.
    assert_eq!(symbol.qualified_name, "module::func");
}

/// Pushing into the recent-activity ring is bounded by the ring's const
/// generic capacity (32). Verify the bound holds even at high push
/// volumes.
#[test]
fn recent_activity_ring_is_bounded_at_compile_time() {
    let now = Timestamp::now();
    let mut builder = WorkspaceContextBuilder::neutral(now);

    for i in 0..200 {
        let win = WindowId::new(format!("win-{i}")).unwrap();
        builder = builder.push_activity(ActivityMarker::WindowFocused {
            window: win,
            at: now,
        });
    }
    let ctx = builder.build();

    let ring = &ctx.recent_activity().ring;
    assert_eq!(ring.capacity(), 32);
    assert_eq!(ring.len(), 32, "ring is full and bounded by N");

    // Verify the contents are the most-recent 32 (focuses 168..200).
    let collected: Vec<&ActivityMarker> = ring.iter().collect();
    if let ActivityMarker::WindowFocused { window, .. } = collected.first().unwrap() {
        assert_eq!(window.as_str(), "win-168");
    } else {
        panic!("first activity should be WindowFocused");
    }
    if let ActivityMarker::WindowFocused { window, .. } = collected.last().unwrap() {
        assert_eq!(window.as_str(), "win-199");
    } else {
        panic!("last activity should be WindowFocused");
    }
}

/// `WorkspaceState::neutral` produces a "no producers attached" baseline
/// — no focused app, neutral biometric, unknown posture, nominal load.
#[test]
fn neutral_state_is_fully_neutral() {
    let now = Timestamp::now();
    let state = WorkspaceState::neutral(now);

    assert_eq!(state.focused_app, None);
    assert_eq!(state.focused_window, None);
    assert_eq!(state.selection, None);
    assert!(state.visible_files.is_empty());
    assert!(state.sensors.is_empty());
    assert_eq!(state.recent_activity.ring.len(), 0);
    assert!(!state.user_state.should_tighten_threshold());
}

/// Querying a context borrows through the Arc — no clone happens on
/// access. We can't observe "did an Arc::clone happen" directly, but we
/// can observe that the `state()` borrow is a `&WorkspaceState`, which
/// is the cheap-query API surface.
#[test]
fn state_accessor_returns_borrow_not_clone() {
    let ctx = WorkspaceContext::neutral(Timestamp::now());

    // `state()` returns &WorkspaceState; cannot coerce a non-borrow to a
    // reference. Compile-time evidence of cheap query.
    let s_ref: &WorkspaceState = ctx.state();
    let s_ref_2: &WorkspaceState = ctx.state();
    // Both borrows are valid; address may differ across calls (compiler
    // may reborrow), but the type system guarantees no allocation.
    assert_eq!(s_ref, s_ref_2);
}
