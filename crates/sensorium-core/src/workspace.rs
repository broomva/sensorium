//! `WorkspaceContext` — the queryable substrate.
//!
//! From `MIL-PROJECT.md` §5.2: the substrate is "the continuously projected
//! workspace state + attention + recent intents + personal history." Pneuma
//! reads it on every parse. Producers update it as observations stream in.
//!
//! ## Two design constraints, made structural
//!
//! From the brief's "interesting design questions":
//!
//! 1. **Cheap to query.** Pneuma reads the substrate on every parse. We
//!    back the context with `Arc<WorkspaceState>` so cloning the context
//!    is bumping a refcount. Field accessors borrow through the `Arc`
//!    with no allocation.
//! 2. **Cheap to take snapshots.** A `WorkspaceSnapshot` is captured at
//!    every directive commit. Snapshot capture is `Arc::clone(&state)`
//!    plus a fresh ID — `O(1)` regardless of state size.
//!
//! ## Snapshot identity & drift detection
//!
//! Snapshots have *two* identity dimensions:
//!
//! - **`WorkspaceSnapshotId`** — a fresh `UUIDv7` minted at capture time.
//!   Sortable by creation time. Two snapshots taken at distinct moments
//!   have distinct IDs even if they observe identical state. Use this
//!   when journaling / referencing.
//! - **Structural identity** — `Arc::ptr_eq` on the inner state. Two
//!   snapshots taken from the same `WorkspaceContext` without any
//!   intervening mutation share the same `Arc` and are
//!   structurally-equal in `O(1)`. Use this for drift detection
//!   between commit-time and dispatch-time:
//!   [`WorkspaceSnapshot::observes_same_state`].
//!
//! The two dimensions answer different questions. ID equality answers "is
//! this the same snapshot record?" Structural equality answers "did the
//! world change between when I committed this and when I'm dispatching?"
//!
//! ## Cross-crate compatibility
//!
//! [`WorkspaceSnapshotId`] is byte-compatible with
//! `pneuma_core::provenance::ContextSnapshotId` (both are `UUIDv7`). The
//! intended bridge is `From<WorkspaceSnapshotId> for ContextSnapshotId`
//! living in a future `pneuma-sensorium` crate; for now, callers can
//! convert via the inner `Uuid`.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entity::{AppId, FileRef, SelectionRef, WindowId, WindowRect};
use crate::ring::RingBuffer;
use crate::sensor::SensorMetadata;
use crate::state::UserState;
use crate::time::Timestamp;

// --- WorkspaceSnapshotId -----------------------------------------------------

/// `UUIDv7` newtype identifying a [`WorkspaceSnapshot`].
///
/// Sortable by creation time. Byte-compatible with
/// `pneuma_core::provenance::ContextSnapshotId` — the structural shape is
/// identical. A future bridge crate will provide `From`/`Into` impls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkspaceSnapshotId(Uuid);

impl WorkspaceSnapshotId {
    /// Mint a fresh `UUIDv7`-backed snapshot ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// Wrap an existing `Uuid` (test fixtures, replay).
    #[must_use]
    pub fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }

    /// Inner UUID, for callers bridging to `pneuma-core`.
    #[must_use]
    pub fn into_inner(self) -> Uuid {
        self.0
    }
}

impl Default for WorkspaceSnapshotId {
    fn default() -> Self {
        Self::new()
    }
}

// --- RecentActivity ----------------------------------------------------------

/// Recent activity log — a bounded ring of recently-observed activity
/// markers.
///
/// "Activity" is intentionally coarse — file accesses, window switches,
/// completed directives. Pneuma's resolver uses it as the default search
/// scope for anaphora resolution: "the file" / "that selection" / "the
/// thing I just edited."
///
/// Capacity is a const generic on the substrate (`32` is the current
/// default; bumping it is a non-breaking change as long as all consumers
/// are updated together — the ring buffer's type encodes it).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecentActivity {
    /// Bounded ring of recent activity markers. Producers push; the ring
    /// evicts oldest when full.
    pub ring: RingBuffer<ActivityMarker, 32>,
}

impl RecentActivity {
    /// Empty activity log.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            ring: RingBuffer::new(),
        }
    }
}

impl Default for RecentActivity {
    fn default() -> Self {
        Self::empty()
    }
}

/// One entry in the recent-activity ring.
///
/// Marker enum — designed to be cheap to construct. Producers stamp
/// these as observations stream in; the substrate doesn't interpret
/// them, only preserves order.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ActivityMarker {
    /// User focused a window.
    WindowFocused {
        /// The window.
        window: WindowId,
        /// When the focus event happened.
        at: Timestamp,
    },
    /// User opened or accessed a file.
    FileAccessed {
        /// The file.
        file: FileRef,
        /// When the access happened.
        at: Timestamp,
    },
    /// User made a selection.
    SelectionMade {
        /// The selection.
        selection: SelectionRef,
        /// When the selection happened.
        at: Timestamp,
    },
    /// A directive committed in the workspace. Carries an opaque ID;
    /// Pneuma resolves it through Lago when needed.
    DirectiveCommitted {
        /// Opaque directive ID. Stringly-typed because we don't depend on
        /// `pneuma-core`.
        directive_id: String,
        /// When the directive committed.
        at: Timestamp,
    },
}

// --- WorkspaceState ----------------------------------------------------------

/// The immutable inner state of the substrate.
///
/// Held inside an `Arc` by [`WorkspaceContext`]. To "modify" the context,
/// callers build a new `WorkspaceState` and `Arc::new` it; the previous
/// state remains live as long as some snapshot references it.
///
/// All fields are populated to "neutral" defaults for sessions where no
/// producer has reported yet. The substrate refuses to expose `Option`s
/// for the load-bearing fields — Pneuma's parser must always be able to
/// answer "what is the focused window?" with *something*, even if the
/// answer is `None`-shaped, so we surface it as `Option` *inside* the
/// state, not on access.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceState {
    /// Currently focused application, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focused_app: Option<AppId>,
    /// Currently focused window, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focused_window: Option<WindowId>,
    /// Bounding rect of the focused window in screen-space.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focused_window_rect: Option<WindowRect>,
    /// Currently active selection, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<SelectionRef>,
    /// Currently visible files (cwd + open editor tabs + bookmarked
    /// files). Producers can keep this small.
    #[serde(default)]
    pub visible_files: Vec<FileRef>,
    /// Bounded ring of recent activity.
    #[serde(default)]
    pub recent_activity: RecentActivity,
    /// Aggregated user state (biometric / posture / load).
    pub user_state: UserState,
    /// Producers attached to this context. Used by Pneuma's router to
    /// know what kinds of evidence are available in the current session.
    #[serde(default)]
    pub sensors: Vec<SensorMetadata>,
    /// When this state was assembled.
    pub assembled_at: Timestamp,
}

impl WorkspaceState {
    /// Construct a neutral baseline state — no focused app, no selection,
    /// no producers. Used as the starting state at session-start.
    #[must_use]
    pub fn neutral(at: Timestamp) -> Self {
        Self {
            focused_app: None,
            focused_window: None,
            focused_window_rect: None,
            selection: None,
            visible_files: Vec::new(),
            recent_activity: RecentActivity::empty(),
            user_state: UserState::neutral(at),
            sensors: Vec::new(),
            assembled_at: at,
        }
    }
}

// --- WorkspaceContext --------------------------------------------------------

/// The cheaply-cloneable substrate handle.
///
/// `WorkspaceContext` is a thin `Arc`-wrapper around [`WorkspaceState`].
/// Cloning it is bumping a refcount. Querying borrows through the `Arc`.
///
/// **Mutation model**: there is no `&mut self` API. To update the
/// context, callers construct a fresh [`WorkspaceState`] (via
/// [`WorkspaceContextBuilder`]) and replace the inner `Arc`. The previous
/// state remains live as long as any snapshot or clone holds it. This is
/// copy-on-write; it makes all queries lock-free at the cost of one
/// allocation per update.
///
/// **Snapshot model**: [`WorkspaceContext::snapshot`] mints a
/// [`WorkspaceSnapshot`] in `O(1)` — fresh ID, `Arc::clone` of state.
/// Drift detection across two snapshots uses
/// [`WorkspaceSnapshot::observes_same_state`].
#[derive(Debug, Clone)]
pub struct WorkspaceContext {
    state: Arc<WorkspaceState>,
}

impl WorkspaceContext {
    /// Construct from an explicit state. The state is moved into an
    /// `Arc`.
    #[must_use]
    pub fn new(state: WorkspaceState) -> Self {
        Self {
            state: Arc::new(state),
        }
    }

    /// A neutral baseline context — no focused app, no producers.
    #[must_use]
    pub fn neutral(at: Timestamp) -> Self {
        Self::new(WorkspaceState::neutral(at))
    }

    /// Capture a snapshot. `O(1)` — fresh ID, `Arc::clone` of state.
    #[must_use]
    pub fn snapshot(&self) -> WorkspaceSnapshot {
        WorkspaceSnapshot {
            id: WorkspaceSnapshotId::new(),
            taken_at: Timestamp::now(),
            state: Arc::clone(&self.state),
        }
    }

    /// Capture a snapshot with explicit timestamp. Used by replay
    /// machinery and tests where wall-clock `now()` would be wrong.
    #[must_use]
    pub fn snapshot_at(&self, taken_at: Timestamp) -> WorkspaceSnapshot {
        WorkspaceSnapshot {
            id: WorkspaceSnapshotId::new(),
            taken_at,
            state: Arc::clone(&self.state),
        }
    }

    /// Borrow the inner state.
    ///
    /// Cheap — no allocation, just dereferences the `Arc`.
    #[must_use]
    pub fn state(&self) -> &WorkspaceState {
        &self.state
    }

    /// Convenience: the currently focused window, if any.
    #[must_use]
    pub fn focused_window(&self) -> Option<&WindowId> {
        self.state.focused_window.as_ref()
    }

    /// Convenience: the currently focused app, if any.
    #[must_use]
    pub fn focused_app(&self) -> Option<&AppId> {
        self.state.focused_app.as_ref()
    }

    /// Convenience: the currently active selection, if any.
    #[must_use]
    pub fn selection(&self) -> Option<&SelectionRef> {
        self.state.selection.as_ref()
    }

    /// Convenience: aggregated user state.
    #[must_use]
    pub fn user_state(&self) -> &UserState {
        &self.state.user_state
    }

    /// Convenience: recent-activity ring.
    #[must_use]
    pub fn recent_activity(&self) -> &RecentActivity {
        &self.state.recent_activity
    }

    /// `true` if `self` and `other` point at the same `Arc<WorkspaceState>`.
    /// Use this for cheap drift-detection between two contexts taken
    /// from the same producer chain.
    #[must_use]
    pub fn shares_state_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.state, &other.state)
    }
}

// --- WorkspaceContextBuilder -------------------------------------------------

/// Builder for [`WorkspaceContext`].
///
/// Producers update the substrate by:
/// 1. Calling [`WorkspaceContext::state`] to read current.
/// 2. Constructing a `WorkspaceContextBuilder` with the current state as
///    a base.
/// 3. Applying their changes.
/// 4. Calling [`WorkspaceContextBuilder::build`] to mint a new
///    `WorkspaceContext`.
///
/// The previous context's snapshots remain valid; their `Arc<WorkspaceState>`
/// is the *previous* state. This means snapshot drift detection with
/// `observes_same_state` correctly reports a change.
#[derive(Debug, Clone)]
pub struct WorkspaceContextBuilder {
    state: WorkspaceState,
}

impl WorkspaceContextBuilder {
    /// Start from a neutral state stamped at `at`.
    #[must_use]
    pub fn neutral(at: Timestamp) -> Self {
        Self {
            state: WorkspaceState::neutral(at),
        }
    }

    /// Start from an existing context's state. Cheap — `Clone` on the
    /// inner state, which may be expensive if `visible_files` is large,
    /// but the substrate keeps that small by convention.
    #[must_use]
    pub fn from_context(context: &WorkspaceContext) -> Self {
        Self {
            state: WorkspaceState::clone(context.state.as_ref()),
        }
    }

    /// Set the focused application.
    #[must_use]
    pub fn with_focused_app(mut self, app: Option<AppId>) -> Self {
        self.state.focused_app = app;
        self
    }

    /// Set the focused window.
    #[must_use]
    pub fn with_focused_window(mut self, window: Option<WindowId>) -> Self {
        self.state.focused_window = window;
        self
    }

    /// Set the focused window rect.
    #[must_use]
    pub fn with_focused_window_rect(mut self, rect: Option<WindowRect>) -> Self {
        self.state.focused_window_rect = rect;
        self
    }

    /// Set the active selection.
    #[must_use]
    pub fn with_selection(mut self, selection: Option<SelectionRef>) -> Self {
        self.state.selection = selection;
        self
    }

    /// Replace the visible-files list.
    #[must_use]
    pub fn with_visible_files(mut self, files: Vec<FileRef>) -> Self {
        self.state.visible_files = files;
        self
    }

    /// Push an activity marker into the ring.
    #[must_use]
    pub fn push_activity(mut self, marker: ActivityMarker) -> Self {
        self.state.recent_activity.ring.push(marker);
        self
    }

    /// Replace the user-state aggregate.
    #[must_use]
    pub fn with_user_state(mut self, user_state: UserState) -> Self {
        self.state.user_state = user_state;
        self
    }

    /// Register a producer.
    #[must_use]
    pub fn with_sensor(mut self, sensor: SensorMetadata) -> Self {
        self.state.sensors.push(sensor);
        self
    }

    /// Set the assembled-at timestamp.
    #[must_use]
    pub fn assembled_at(mut self, at: Timestamp) -> Self {
        self.state.assembled_at = at;
        self
    }

    /// Build a fresh [`WorkspaceContext`]. The previous context's
    /// snapshots remain valid (their `Arc<WorkspaceState>` is the older
    /// state).
    #[must_use]
    pub fn build(self) -> WorkspaceContext {
        WorkspaceContext::new(self.state)
    }
}

// --- WorkspaceSnapshot -------------------------------------------------------

/// A point-in-time projection of the workspace.
///
/// Holds a fresh [`WorkspaceSnapshotId`] (sortable, journal-friendly), a
/// `taken_at` timestamp, and an `Arc<WorkspaceState>` pointing to the
/// state observed at capture time.
///
/// Snapshots are the substrate's contribution to the directive's
/// `ContextRef` — Pneuma copies the `WorkspaceSnapshotId` into the
/// committed directive. At dispatch time, executors compare the
/// committed snapshot to a fresh snapshot via
/// [`WorkspaceSnapshot::observes_same_state`] to detect drift.
#[derive(Debug, Clone)]
pub struct WorkspaceSnapshot {
    /// Fresh `UUIDv7` minted at capture.
    pub id: WorkspaceSnapshotId,
    /// When the snapshot was captured.
    pub taken_at: Timestamp,
    /// Pointer to the captured state. `Arc` is the cheap-snapshot
    /// trick: capture is `Arc::clone` regardless of state size.
    pub state: Arc<WorkspaceState>,
}

impl WorkspaceSnapshot {
    /// `true` if `self` and `other` observe the same underlying
    /// `Arc<WorkspaceState>`.
    ///
    /// Use this for drift detection: take a fresh snapshot at
    /// dispatch time, compare to the snapshot bound to the directive at
    /// commit time, and refuse the dispatch if they differ.
    #[must_use]
    pub fn observes_same_state(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.state, &other.state)
    }
}

impl PartialEq for WorkspaceSnapshot {
    /// Snapshot equality is *id equality* — two snapshots are the same
    /// snapshot record iff they share an ID. Use
    /// [`WorkspaceSnapshot::observes_same_state`] for structural equality.
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for WorkspaceSnapshot {}

// `Serialize` / `Deserialize` for `WorkspaceSnapshot` would have to handle
// the `Arc` somehow; we punt for v0.2. Snapshots are intended for
// in-process use; what crosses the wire is the `WorkspaceSnapshotId` plus
// (separately) the `WorkspaceState` it references, journaled by Lago.
