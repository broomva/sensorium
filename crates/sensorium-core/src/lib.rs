//! # sensorium-core
//!
//! Types-only foundation for the **Sensorium** subsystem of the Life Agent OS.
//!
//! Sensorium is the *exteroceptive* substrate — the world projected as typed
//! observations the agent can query. It is the dual of [Vigil][vigil], which
//! is *proprioceptive* (the agent observing itself).
//!
//! [vigil]: https://github.com/broomva/life
//!
//! This crate defines the substrate types only. Producers (`sensorium-vision`,
//! `sensorium-voice`, `sensorium-gaze`, `sensorium-context`, etc.) are separate
//! crates that depend on this one. Consumers (`pneuma-parser`, `pneuma-router`,
//! `pneuma-resolver`) read from it on every parse.
//!
//! ## Why a types-only crate
//!
//! The substrate is read on **every parse** by the Pneuma stack. Allocations
//! and indirections matter. Decoupling types from observers lets us:
//!
//! - Keep the dependency graph acyclic: `pneuma-router` depends on
//!   `sensorium-core`, never on a producer.
//! - Compile in `no_std`-curious environments later (no I/O dragged in).
//! - Test the substrate model in isolation, with hand-built `WorkspaceContext`
//!   instances, before any sensor is wired.
//!
//! ## The five load-bearing types
//!
//! 1. [`WorkspaceContext`] — the cheaply-cloneable, structurally-shared
//!    substrate. `Arc`-backed so taking a snapshot is `O(1)`.
//! 2. [`WorkspaceSnapshot`] — a point-in-time projection with stable identity,
//!    used by Pneuma for drift detection between commit-time and dispatch-time.
//! 3. [`PrimitiveToken`] — the seven-primitive event taxonomy
//!    (reference / predication / modulation / relation / approval / attention /
//!    state) that producers emit and Pneuma consumes.
//! 4. [`Tagged<T>`] — universal provenance wrapper. Every observation carries
//!    `SensorId`, `Calibration`, and timestamps. **Nothing in the substrate is
//!    bare.**
//! 5. [`PrivacyTier`] / [`LocalOnly`] — privacy as a structural property, not a
//!    runtime check. The substrate is local-only by hard architectural
//!    requirement; `LocalOnly<T>` makes that visible to the type system.
//!
//! ## Cross-crate compatibility with `pneuma-core`
//!
//! [`WorkspaceSnapshotId`] is a `UUIDv7` newtype, structurally identical to
//! `pneuma_core::provenance::ContextSnapshotId`. The two crates do not depend
//! on each other; a future `pneuma-sensorium` bridge crate will provide
//! `From`/`Into` impls. The wire format is byte-compatible today.
//!
//! Similarly, [`AppId`], [`WindowId`], [`FileRef`], [`SymbolRef`], and
//! [`SelectionRef`] live here as the canonical home for workspace-entity types
//! — observed by sensors, referenced by directives. `pneuma-core::referent`
//! currently mirrors them; later versions will move to a shared definition.
//!
//! ## What is not in this crate
//!
//! No I/O, no observers, no async runtime, no platform code. Those live in
//! producer crates. This crate is `sensors observe → typed substrate`, the
//! types only. See [MIL-PROJECT.md][spec] §11 for the full build order.
//!
//! [spec]: https://github.com/broomva/sensorium

#![doc = include_str!("../README.md")]

pub mod attention;
pub mod biometric;
pub mod entity;
pub mod error;
pub mod posture;
pub mod primitive;
pub mod privacy;
pub mod provenance;
pub mod ring;
pub mod sensor;
pub mod state;
pub mod time;
pub mod token;
pub mod workspace;

// --- Public re-exports: the load-bearing surface. ----------------------------

pub use attention::{Fixation, GazeFixation, GazePoint, GazeSample};
pub use biometric::{ArousalLevel, BiometricSnapshot, HeartRate, SkinConductance};
pub use entity::{
    AppId, FileRef, MimeType, SelectionRef, SymbolRef, TextSpan, Uri, WindowId, WindowRect,
};
pub use error::{Result, SensoriumError};
pub use posture::{Posture, PostureSnapshot, PresenceLevel};
pub use primitive::PrimitiveKind;
pub use privacy::{LocalOnly, PrivacyTier, Redacted, RedactionReason};
pub use provenance::{Provenance, Tagged};
pub use ring::RingBuffer;
pub use sensor::{Calibration, CalibrationStatus, SensorId, SensorKind, SensorMetadata};
pub use state::{CognitiveLoad, UserState};
pub use time::{Monotonic, Timestamp};
pub use token::{ApprovalEvent, AttentionEvent, ModulationEvent, PrimitiveToken, RelationEvent};
pub use workspace::{
    ActivityMarker, RecentActivity, WorkspaceContext, WorkspaceContextBuilder, WorkspaceSnapshot,
    WorkspaceSnapshotId, WorkspaceState,
};

/// The grammar version this crate ships against.
///
/// Wire-compatible with `pneuma-core` `GRAMMAR_VERSION`. Kept in lockstep —
/// when one bumps, the other must bump. See `MIL-PROJECT.md` §6 for the
/// full version-management story.
pub const GRAMMAR_VERSION: &str = "0.2.0";
