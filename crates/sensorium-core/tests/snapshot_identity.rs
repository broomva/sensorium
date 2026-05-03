//! Snapshot identity & drift detection.
//!
//! Properties under test (cross-references to `MIL-PROJECT.md`):
//!
//! - **§5.2** "Continuously projected workspace state" — substrate is
//!   queryable cheaply; snapshots are taken at every commit.
//! - **§6.1** `context: Option<ContextRef>` — directives carry a snapshot
//!   ID that executors can re-validate at dispatch time.
//! - **§6.3 guarantee 5** "Every committed directive carries the workspace
//!   snapshot it was committed against."
//!
//! What this means structurally: snapshots must distinguish "same record"
//! (ID equality) from "same observed state" (structural equality). The
//! tests here verify each axis independently.

use std::sync::Arc;

use sensorium_core::{Timestamp, WorkspaceContext, WorkspaceContextBuilder, WorkspaceState};

/// Two snapshots taken from the same un-mutated context share the
/// underlying `Arc<WorkspaceState>`. This is the cheap-snapshot guarantee:
/// capture is `Arc::clone`, not a deep copy.
#[test]
fn snapshots_from_same_context_share_state() {
    let ctx = WorkspaceContext::neutral(Timestamp::now());

    let a = ctx.snapshot();
    let b = ctx.snapshot();

    assert!(
        a.observes_same_state(&b),
        "snapshots from the same context must share an Arc"
    );
}

/// Two snapshots from the same context still get distinct IDs. ID
/// equality is "same record"; structural equality is "same observation".
/// Conflating them is the whole bug class snapshot identity exists to
/// prevent.
#[test]
fn snapshots_have_distinct_ids_even_when_structurally_equal() {
    let ctx = WorkspaceContext::neutral(Timestamp::now());

    let a = ctx.snapshot();
    let b = ctx.snapshot();

    assert_ne!(a.id, b.id, "every snapshot capture mints a fresh UUIDv7");
    assert!(a.observes_same_state(&b), "but they observe the same state");
}

/// Building a fresh context from the previous one produces a different
/// `Arc<WorkspaceState>`, so drift detection across the rebuild
/// reports `false`. This is the substrate's contribution to executor
/// drift checks.
#[test]
fn rebuild_breaks_structural_identity() {
    let ctx_a = WorkspaceContext::neutral(Timestamp::now());
    let snapshot_a = ctx_a.snapshot();

    // A producer "observes" something new and rebuilds the context.
    let ctx_b = WorkspaceContextBuilder::from_context(&ctx_a)
        .assembled_at(Timestamp::now())
        .build();
    let snapshot_b = ctx_b.snapshot();

    assert!(
        !snapshot_a.observes_same_state(&snapshot_b),
        "after a rebuild, the substrate observes a new Arc"
    );
}

/// Cloning the context (cheap operation) shares the `Arc`. Snapshots
/// taken from either clone share state. This is the producer pattern:
/// cheap to fan out the same context to multiple consumers.
#[test]
fn cloned_contexts_share_state() {
    let ctx = WorkspaceContext::neutral(Timestamp::now());
    let cloned = ctx.clone();

    assert!(ctx.shares_state_with(&cloned));

    let s_orig = ctx.snapshot();
    let s_clone = cloned.snapshot();
    assert!(s_orig.observes_same_state(&s_clone));
}

/// Snapshot taken via `snapshot_at` honors the explicit timestamp.
/// Wall-clock `now()` is the wrong choice in replay scenarios; the
/// substrate exposes the explicit form.
#[test]
fn snapshot_at_uses_explicit_timestamp() {
    let ctx = WorkspaceContext::neutral(Timestamp::now());

    let explicit =
        Timestamp::from_millis_utc(1_700_000_000_000).expect("epoch millis are in range");
    let snapshot = ctx.snapshot_at(explicit);

    assert_eq!(snapshot.taken_at, explicit);
}

/// Snapshots are PartialEq by ID, *not* by structural state. This is the
/// "same record" axis. Confirm explicitly so future maintainers don't
/// flip the meaning by accident.
#[test]
fn snapshot_partial_eq_is_id_equality() {
    let ctx = WorkspaceContext::neutral(Timestamp::now());
    let a = ctx.snapshot();
    let b = ctx.snapshot();

    // Distinct IDs → distinct snapshots, even though they observe the
    // same state.
    assert_ne!(a, b);
    assert!(a.observes_same_state(&b));
}

/// Constructing a context from an explicit `WorkspaceState` honors the
/// fields. (Sanity check on the constructor surface.)
#[test]
fn explicit_state_round_trips_through_context() {
    let at = Timestamp::now();
    let state = WorkspaceState::neutral(at);
    let ctx = WorkspaceContext::new(state.clone());

    assert_eq!(ctx.state(), &state);
}

/// Distinct contexts holding equal-by-value states do NOT share the
/// `Arc`. This is the fork case — two producers independently
/// constructed equivalent state. Pneuma must treat these as drift,
/// because the *provenance* is different.
#[test]
fn equivalent_states_constructed_independently_do_not_share_arc() {
    let at = Timestamp::now();
    let ctx_a = WorkspaceContext::new(WorkspaceState::neutral(at));
    let ctx_b = WorkspaceContext::new(WorkspaceState::neutral(at));

    assert_eq!(ctx_a.state(), ctx_b.state(), "by-value equality holds");
    assert!(
        !ctx_a.shares_state_with(&ctx_b),
        "but they're independent Arcs — drift detection must fire"
    );
}

/// Snapshot's underlying state is reachable by `Arc::ptr_eq` against the
/// originating context. This is the underlying mechanism that
/// `observes_same_state` is built on; testing it explicitly nails down
/// the cheap-clone story.
#[test]
fn snapshot_arc_is_pointer_equal_to_context_state() {
    let ctx = WorkspaceContext::neutral(Timestamp::now());
    let snapshot = ctx.snapshot();

    // The snapshot's `state` should be Arc-pointer-equal to a fresh
    // `Arc::clone(ctx.state)`. Since we don't have direct access to the
    // context's inner Arc, take two snapshots and verify ptr_eq.
    let snapshot_2 = ctx.snapshot();
    assert!(Arc::ptr_eq(&snapshot.state, &snapshot_2.state));
}
