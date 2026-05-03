//! [`ManualObserver`] — programmatic, lock-protected workspace context
//! provider.

use std::sync::{Arc, Mutex};

use sensorium_core::{
    ActivityMarker, AppId, FileRef, SelectionRef, SensorMetadata, Timestamp, WindowId, WindowRect,
    WorkspaceContext, WorkspaceContextBuilder,
};

use crate::observer::Observer;

/// A programmatically-driven [`Observer`].
///
/// Internal state is a `Mutex<WorkspaceContext>`. Producer-side
/// methods (`set_focused_file`, `push_activity`, `rebuild_with`)
/// rebuild the context via the substrate's copy-on-write builder.
/// Reader-side methods (`current`, `snapshot`) acquire the mutex
/// briefly and clone the (cheap, `Arc`-backed) context.
///
/// ## When to use
///
/// - Tests that need to drive the substrate through specific states.
/// - Scripted demos where input comes from a fixture, not real I/O.
/// - The Tier 2 `pneuma-demo` for its rename flow.
///
/// ## When NOT to use
///
/// - Real production observation (use [`crate::FsObserver`] for
///   filesystem changes; macOS workspace observation is a planned
///   `sensorium-context-macos` crate).
pub struct ManualObserver {
    inner: Arc<Mutex<WorkspaceContext>>,
}

impl ManualObserver {
    /// Construct a manual observer with a neutral context stamped at
    /// `at`.
    #[must_use]
    pub fn new(at: Timestamp) -> Self {
        Self {
            inner: Arc::new(Mutex::new(WorkspaceContext::neutral(at))),
        }
    }

    /// Construct from an explicit initial context.
    #[must_use]
    pub fn from_context(ctx: WorkspaceContext) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ctx)),
        }
    }

    /// Set the focused application. Rebuilds the context.
    pub fn set_focused_app(&self, app: Option<AppId>) {
        self.rebuild_with(|b| b.with_focused_app(app));
    }

    /// Set the focused window. Rebuilds the context.
    pub fn set_focused_window(&self, window: Option<WindowId>) {
        self.rebuild_with(|b| b.with_focused_window(window));
    }

    /// Set the focused window's bounding rect.
    pub fn set_focused_window_rect(&self, rect: Option<WindowRect>) {
        self.rebuild_with(|b| b.with_focused_window_rect(rect));
    }

    /// Set the active selection. Rebuilds the context.
    pub fn set_selection(&self, selection: Option<SelectionRef>) {
        self.rebuild_with(|b| b.with_selection(selection));
    }

    /// Replace the visible-files list. Rebuilds the context.
    pub fn set_visible_files(&self, files: Vec<FileRef>) {
        self.rebuild_with(|b| b.with_visible_files(files));
    }

    /// Convenience: announce a file is focused — sets `visible_files`
    /// to `[file]` and (optionally) records an activity marker.
    pub fn set_focused_file(&self, file: FileRef, push_activity: bool) {
        let now = Timestamp::now();
        self.rebuild_with(|mut b| {
            b = b.with_visible_files(vec![file.clone()]);
            if push_activity {
                b = b.push_activity(ActivityMarker::FileAccessed { file, at: now });
            }
            b
        });
    }

    /// Push an activity marker into the recent-activity ring.
    pub fn push_activity(&self, marker: ActivityMarker) {
        self.rebuild_with(|b| b.push_activity(marker));
    }

    /// Register a sensor that's contributing to this context.
    pub fn register_sensor(&self, sensor: SensorMetadata) {
        self.rebuild_with(|b| b.with_sensor(sensor));
    }

    /// Rebuild the context with an arbitrary builder transform.
    /// All other producer-side methods are convenience wrappers
    /// around this.
    pub fn rebuild_with<F>(&self, f: F)
    where
        F: FnOnce(WorkspaceContextBuilder) -> WorkspaceContextBuilder,
    {
        let mut guard = self
            .inner
            .lock()
            .expect("ManualObserver mutex was poisoned");
        let next_builder =
            f(WorkspaceContextBuilder::from_context(&guard)).assembled_at(Timestamp::now());
        *guard = next_builder.build();
    }

    /// Reset the context to a neutral baseline stamped at `at`.
    pub fn reset(&self, at: Timestamp) {
        let mut guard = self
            .inner
            .lock()
            .expect("ManualObserver mutex was poisoned");
        *guard = WorkspaceContext::neutral(at);
    }
}

impl Observer for ManualObserver {
    fn current(&self) -> WorkspaceContext {
        self.inner
            .lock()
            .expect("ManualObserver mutex was poisoned")
            .clone()
    }
}

impl Clone for ManualObserver {
    /// Cloning a `ManualObserver` shares its `Arc<Mutex<...>>` —
    /// both clones see the same context and producer-side updates
    /// propagate to all clones. This is intentional: it's the
    /// natural way to fan an observer out to multiple consumers.
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}
