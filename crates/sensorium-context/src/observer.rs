//! The `Observer` trait — every producer implements this.

use sensorium_core::{WorkspaceContext, WorkspaceSnapshot};

/// Source of `WorkspaceContext` updates.
///
/// Pull-based: consumers call [`Observer::current`] to read the
/// latest context, or [`Observer::snapshot`] to take a typed
/// snapshot for journal cross-reference.
///
/// **Thread safety:** implementations must be `Send + Sync` so the
/// demo / pneuma-router / multi-consumer paths can share an
/// observer without external locking. Each implementor's docs
/// describe its concurrency model.
pub trait Observer: Send + Sync {
    /// Borrow the latest context.
    ///
    /// Returns an *owned* `WorkspaceContext` — `Arc`-backed under the
    /// hood so the clone is cheap. Consumers may hold this for as
    /// long as they like; the underlying `Arc<WorkspaceState>`
    /// keeps the state alive even if the observer's internal state
    /// pointer has moved on.
    fn current(&self) -> WorkspaceContext;

    /// Take a fresh snapshot. Convenience wrapper over
    /// `self.current().snapshot()`.
    fn snapshot(&self) -> WorkspaceSnapshot {
        self.current().snapshot()
    }
}
