//! # sensorium-context
//!
//! Workspace observer providers — the first producer layer for the
//! Sensorium substrate, Phase 1.1 of the MIL build.
//!
//! ## What this crate is for
//!
//! `sensorium-core` defines the substrate types (`WorkspaceContext`,
//! `WorkspaceSnapshot`, `ActivityMarker`). This crate defines the
//! producers that *populate* the substrate: an [`Observer`] trait and
//! two implementations that the demo + downstream Pneuma can consume.
//!
//! ## Why pull, not push
//!
//! The pneuma-router consumes the substrate on every parse, but the
//! v0.2 demo flow has only one substrate query per directive
//! (snapshot taken at commit time). Pull is sufficient. v0.3 will
//! add a `subscribe` method for the production HUD that needs every
//! state change at 30 Hz.

#![doc = include_str!("../README.md")]

mod fs_observer;
mod manual_observer;
mod observer;

pub use fs_observer::{FsObserver, FsObserverError};
pub use manual_observer::ManualObserver;
pub use observer::Observer;
