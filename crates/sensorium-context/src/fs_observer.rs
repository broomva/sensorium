//! [`FsObserver`] — filesystem-watching observer.
//!
//! Wraps [`notify`] to detect file events under a watched path. On
//! every event, pushes an `ActivityMarker::FileAccessed` into the
//! shared context. Spawns one watcher thread per instance; thread is
//! joined on `Drop`.
//!
//! ## What this proves
//!
//! Tier 2 Risk #6 from the synthesis doc — *snapshot drift detection
//! must work under real concurrency* — is concretely answerable with
//! this observer. A test that watches a tempdir, writes to a file
//! in another thread, and then takes a snapshot before vs. after
//! demonstrates that:
//!
//! - The substrate updates (new `Arc<WorkspaceState>` allocated).
//! - Snapshots before and after the rebuild are NOT
//!   `observes_same_state` (Arc::ptr_eq fails).
//! - The drift helper from `pneuma_router::drift_detected` fires
//!   correctly.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

use notify::event::EventKind;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use thiserror::Error;

use sensorium_core::{
    ActivityMarker, FileRef, Timestamp, WorkspaceContext, WorkspaceContextBuilder,
};

use crate::manual_observer::ManualObserver;
use crate::observer::Observer;

/// Errors raised by [`FsObserver`].
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FsObserverError {
    /// `notify` could not start watching the path.
    #[error("notify error: {0}")]
    Notify(#[from] notify::Error),

    /// I/O error during setup.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Filesystem-watching observer.
///
/// Watches a path recursively. On every detected event, updates the
/// shared `WorkspaceContext` by pushing a `FileAccessed` activity
/// marker (and refreshing `visible_files` if `track_visible` is set).
///
/// ## Concurrency model
///
/// The observer owns a [`ManualObserver`] internally and exposes its
/// snapshot/`current` methods. A background thread reads filesystem
/// events from `notify` and translates them to `ManualObserver`
/// updates. Consumers read the substrate via the [`Observer`] trait
/// — same as for `ManualObserver`.
///
/// Calling [`FsObserver::stop`] (or dropping the observer) signals
/// the watcher thread to exit.
///
/// ## What it does NOT do
///
/// - Distinguish create vs modify vs remove. v0.2 funnels everything
///   through `FileAccessed`; v0.3 will discriminate.
/// - Persist state across restarts.
/// - Debounce duplicate events. `notify` may emit several events
///   per save; we surface them all.
pub struct FsObserver {
    inner: ManualObserver,
    // The watcher must outlive the thread; we keep it alive in a
    // Mutex so the watcher's drop can run during `stop`.
    _watcher: Arc<Mutex<Option<RecommendedWatcher>>>,
    stop_tx: mpsc::Sender<()>,
    handle: Mutex<Option<thread::JoinHandle<()>>>,
}

impl FsObserver {
    /// Watch `path` recursively, refreshing the substrate on each
    /// filesystem event. If `track_visible` is true, every event also
    /// rebuilds the `visible_files` list to `[path-of-event]`.
    ///
    /// Returns immediately after the watcher is set up; events flow
    /// asynchronously in a background thread.
    pub fn watch(path: impl AsRef<Path>, track_visible: bool) -> Result<Self, FsObserverError> {
        let path = path.as_ref().to_path_buf();

        let inner = ManualObserver::new(Timestamp::now());
        let manual_for_thread = inner.clone();

        let (event_tx, event_rx) = mpsc::channel::<notify::Result<notify::Event>>();
        let (stop_tx, stop_rx) = mpsc::channel::<()>();

        let mut watcher = RecommendedWatcher::new(
            move |res: notify::Result<notify::Event>| {
                let _ = event_tx.send(res);
            },
            Config::default(),
        )?;
        watcher.watch(&path, RecursiveMode::Recursive)?;

        let watcher_handle = Arc::new(Mutex::new(Some(watcher)));

        let handle = thread::Builder::new()
            .name(format!("sensorium-fsobserver:{}", path.display()))
            .spawn(move || {
                Self::event_loop(&manual_for_thread, &event_rx, &stop_rx, track_visible);
            })?;

        Ok(Self {
            inner,
            _watcher: watcher_handle,
            stop_tx,
            handle: Mutex::new(Some(handle)),
        })
    }

    fn event_loop(
        manual: &ManualObserver,
        events: &mpsc::Receiver<notify::Result<notify::Event>>,
        stop_rx: &mpsc::Receiver<()>,
        track_visible: bool,
    ) {
        loop {
            // Check for stop signal first so the loop exits promptly
            // even if events are queued.
            if stop_rx.try_recv().is_ok() {
                return;
            }
            // `clippy::match_same_arms` is allowed: `Ok(Err(_))`
            // (notify-side error) and `Err(Timeout)` (channel idle)
            // are conceptually distinct even though both are no-ops
            // in v0.2 — v0.3 will surface notify errors via vigil.
            #[allow(clippy::match_same_arms)]
            match events.recv_timeout(Duration::from_millis(100)) {
                Ok(Ok(ev)) => Self::handle_event(manual, ev, track_visible),
                Ok(Err(_)) => {}
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
    }

    fn handle_event(manual: &ManualObserver, event: notify::Event, track_visible: bool) {
        // Skip access events; they fire on every read and would flood
        // the activity ring.
        if matches!(event.kind, EventKind::Access(_)) {
            return;
        }
        let now = Timestamp::now();
        let paths: Vec<PathBuf> = event.paths;
        manual.rebuild_with(|mut b: WorkspaceContextBuilder| {
            for p in &paths {
                b = b.push_activity(ActivityMarker::FileAccessed {
                    file: FileRef::new(p.clone()),
                    at: now,
                });
            }
            if track_visible {
                let visible: Vec<FileRef> = paths.iter().map(|p| FileRef::new(p.clone())).collect();
                b = b.with_visible_files(visible);
            }
            b
        });
    }

    /// Signal the watcher thread to exit. Returns when the thread
    /// has joined. Idempotent.
    pub fn stop(&self) {
        let _ = self.stop_tx.send(());
        if let Some(handle) = self.handle.lock().expect("handle mutex poisoned").take() {
            let _ = handle.join();
        }
    }

    /// Borrow the underlying `ManualObserver` — useful for tests or
    /// for combining filesystem-driven and programmatic updates.
    #[must_use]
    pub fn manual(&self) -> &ManualObserver {
        &self.inner
    }
}

impl Drop for FsObserver {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Observer for FsObserver {
    fn current(&self) -> WorkspaceContext {
        self.inner.current()
    }
}
