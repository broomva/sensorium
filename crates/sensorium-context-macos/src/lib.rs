//! # sensorium-context-macos
//!
//! macOS workspace observer for the Sensorium substrate.
//!
//! Polls `NSWorkspace.shared.frontmostApplication` once per second
//! on a background thread and updates a shared [`WorkspaceContext`]
//! so consumers (notably `pneuma-resolver`) can read which app is
//! currently focused.
//!
//! Step #15 of `MIL-PROJECT.md` §11.2. Sibling of
//! `sensorium_context::FsObserver` — same trait shape, same
//! background-thread + Drop-join pattern, different signal source.
//!
//! ## What this crate is
//!
//! - [`MacOsWorkspaceObserver`] — implements
//!   [`sensorium_context::Observer`] by polling NSWorkspace.
//! - On non-macOS, the same struct compiles as a stub: it constructs
//!   without spawning a thread, and `current()` returns the initial
//!   empty context.
//!
//! ## What this crate is NOT
//!
//! - **Not an Accessibility client.** Focused-window queries via the
//!   AX API need user-granted permission and live in v0.3.
//! - **Not notification-driven.** v0.2 polls at 1Hz. v0.3 will
//!   subscribe to `NSWorkspaceDidActivateApplicationNotification`.
//! - **Not a `Send + Sync` value type.** Like `FsObserver`, this owns
//!   a background thread; consumers reference it through the
//!   `Observer` trait.
//!
//! ## Concurrency model
//!
//! Same as `FsObserver`: own a [`ManualObserver`] internally, spawn
//! one polling thread, signal stop via mpsc channel, join on Drop.

#![doc = include_str!("../README.md")]

use std::sync::{Mutex, mpsc};
use std::thread;
use std::time::Duration;

use sensorium_context::{ManualObserver, Observer};
use sensorium_core::{Timestamp, WorkspaceContext};
use thiserror::Error;

#[cfg(target_os = "macos")]
mod macos;

// --- Error -----------------------------------------------------------------

/// Errors raised by [`MacOsWorkspaceObserver`].
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum MacOsObserverError {
    /// Failed to spawn the polling thread.
    #[error("failed to spawn polling thread: {0}")]
    ThreadSpawn(std::io::Error),
}

// --- Observer ---------------------------------------------------------------

/// Polling observer for the macOS workspace state.
///
/// On `target_os = "macos"`, spawns a background thread that polls
/// `NSWorkspace.shared.frontmostApplication` at the configured period
/// and updates the inner [`ManualObserver`]'s [`WorkspaceContext`]
/// whenever the focused app changes.
///
/// On other platforms, the struct constructs cleanly but the
/// background thread is never spawned. `current()` returns an empty
/// context. This keeps the crate cross-platform-compilable so
/// downstream crates don't need their own cfg gates.
pub struct MacOsWorkspaceObserver {
    inner: ManualObserver,
    stop_tx: Option<mpsc::Sender<()>>,
    handle: Mutex<Option<thread::JoinHandle<()>>>,
}

impl MacOsWorkspaceObserver {
    /// Construct an observer that polls every `period` (typically
    /// `Duration::from_secs(1)`).
    pub fn start(period: Duration) -> Result<Self, MacOsObserverError> {
        let inner = ManualObserver::new(Timestamp::now());

        #[cfg(target_os = "macos")]
        {
            let manual = inner.clone();
            let (stop_tx, stop_rx) = mpsc::channel::<()>();
            let handle = thread::Builder::new()
                .name("sensorium-macos-workspace".to_owned())
                .spawn(move || {
                    Self::event_loop(&manual, &stop_rx, period);
                })
                .map_err(MacOsObserverError::ThreadSpawn)?;
            Ok(Self {
                inner,
                stop_tx: Some(stop_tx),
                handle: Mutex::new(Some(handle)),
            })
        }

        // Non-macOS stub: no thread, no stop channel.
        #[cfg(not(target_os = "macos"))]
        {
            // Suppress unused-variable warning on non-macOS.
            let _ = period;
            Ok(Self {
                inner,
                stop_tx: None,
                handle: Mutex::new(None),
            })
        }
    }

    /// Convenience: start a default-period observer (1Hz).
    pub fn start_default() -> Result<Self, MacOsObserverError> {
        Self::start(Duration::from_secs(1))
    }

    /// Stop the polling thread and join it. Idempotent. Drop runs
    /// this automatically.
    pub fn stop(&self) {
        if let Some(tx) = &self.stop_tx {
            let _ = tx.send(());
        }
        if let Ok(mut guard) = self.handle.lock() {
            if let Some(handle) = guard.take() {
                let _ = handle.join();
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn event_loop(manual: &ManualObserver, stop_rx: &mpsc::Receiver<()>, period: Duration) {
        // Poll-then-sleep loop. We use recv_timeout(period) so a stop
        // signal exits the loop promptly without waiting out the
        // remainder of the poll interval.
        loop {
            macos::poll_once(manual);
            match stop_rx.recv_timeout(period) {
                // Stop signal — exit loop.
                Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => return,
                // Timeout means no stop signal — continue polling.
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        }
    }
}

impl Drop for MacOsWorkspaceObserver {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Observer for MacOsWorkspaceObserver {
    fn current(&self) -> WorkspaceContext {
        self.inner.current()
    }

    fn snapshot(&self) -> sensorium_core::WorkspaceSnapshot {
        self.inner.snapshot()
    }
}
