//! Workspace entity types — the things the substrate sees.
//!
//! These are the ID types that show up in [`crate::token::PrimitiveToken`]
//! references and in directives' [`Referent`][referent] enum. They are
//! *observed* identifiers — they exist because a sensor saw them — not
//! universally-unique cryptographic IDs.
//!
//! [referent]: https://github.com/broomva/pneuma
//!
//! ## Coordination with `pneuma-core`
//!
//! `pneuma-core::referent::{AppId, WindowId, FileRef, SelectionRef, SymbolRef}`
//! are structurally identical mirrors of these. The two crates do not yet
//! share a dependency; the canonical home will eventually be here. For v0.2
//! we accept the duplication and document a `From`/`Into` bridge as the
//! migration path.
//!
//! ## Why string newtypes for IDs
//!
//! App / window / symbol identifiers are *platform-defined* — bundle IDs on
//! macOS, `wmclass` on X11, accessibility-tree paths in some toolkits. The
//! substrate doesn't know what shape they take; it just preserves them
//! verbatim and asserts they are non-empty. Use [`AppId::new`] etc. for
//! checked construction.

use serde::{Deserialize, Serialize};

use crate::error::{Result, SensoriumError};

// --- Helpers ------------------------------------------------------------------

/// Reject empty / whitespace-only ID strings at construction time. IDs that
/// fail this check are sensor bugs and should be rejected at the source.
fn validate_id(field: &'static str, raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(SensoriumError::EmptyIdentifier { field });
    }
    Ok(trimmed.to_owned())
}

// --- Application -------------------------------------------------------------

/// Platform-specific application identifier.
///
/// Examples:
/// - macOS: `"com.apple.Safari"` (bundle ID)
/// - Linux/X11: `"firefox"` (`wmclass`)
/// - Windows: `"Notepad"` (process name) or `"Microsoft.Office.WORD"` (AUMID)
///
/// The substrate does not interpret the ID; it round-trips it through the
/// token stream and the snapshot.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AppId(String);

impl AppId {
    /// Construct an `AppId`, rejecting empty or whitespace-only input.
    pub fn new(raw: impl Into<String>) -> Result<Self> {
        Ok(Self(validate_id("AppId", &raw.into())?))
    }

    /// Construct without validation. For tests and for sensors that have
    /// already validated.
    #[must_use]
    pub fn from_string_unchecked(raw: String) -> Self {
        Self(raw)
    }

    /// Borrow the underlying ID string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// --- Window ------------------------------------------------------------------

/// Platform-specific window identifier, scoped within an application.
///
/// Often a numeric handle (X11 `XID`, macOS `CGWindowID`, Windows `HWND`)
/// rendered as a string for portability. May also be a structural path like
/// `"chrome://tabs/12"`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WindowId(String);

impl WindowId {
    /// Construct a `WindowId`, rejecting empty or whitespace-only input.
    pub fn new(raw: impl Into<String>) -> Result<Self> {
        Ok(Self(validate_id("WindowId", &raw.into())?))
    }

    /// Construct without validation. For tests and trusted sensor pipelines.
    #[must_use]
    pub fn from_string_unchecked(raw: String) -> Self {
        Self(raw)
    }

    /// Borrow the underlying ID string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Window bounding rectangle in screen coordinates, pixel units.
///
/// Coordinates are top-left origin, X axis right, Y axis down — matching every
/// major desktop OS. Sensors that observe in a different convention must
/// translate before stamping.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WindowRect {
    /// X coordinate of the top-left corner.
    pub x: i32,
    /// Y coordinate of the top-left corner.
    pub y: i32,
    /// Width in pixels (always non-negative; zero is allowed for
    /// minimized/zero-sized windows).
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl WindowRect {
    /// Test whether a screen-space point falls inside this rectangle.
    /// Used for gaze hit-testing.
    #[must_use]
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.x
            && y >= self.y
            && x < self.x.saturating_add_unsigned(self.width)
            && y < self.y.saturating_add_unsigned(self.height)
    }
}

// --- File --------------------------------------------------------------------

/// MIME type of an observed file or selection.
///
/// Stored as the canonical lowercased string (`"text/plain"`,
/// `"application/json"`). Construction normalizes the case so equality is
/// well-defined.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MimeType(String);

impl MimeType {
    /// Construct a `MimeType`, lowercasing and trimming. Rejects empty input.
    pub fn new(raw: impl AsRef<str>) -> Result<Self> {
        let trimmed = raw.as_ref().trim();
        if trimmed.is_empty() {
            return Err(SensoriumError::EmptyIdentifier { field: "MimeType" });
        }
        Ok(Self(trimmed.to_lowercase()))
    }

    /// Borrow the canonical type string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Reference to a file the substrate has observed.
///
/// `path` is the canonical platform path. `mime` is optional — the file may
/// not have been read yet. The reference does *not* include file contents;
/// content is fetched lazily by Pneuma resolvers when needed.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileRef {
    /// Absolute path on the local filesystem.
    pub path: std::path::PathBuf,
    /// MIME type if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime: Option<MimeType>,
}

impl FileRef {
    /// Construct a [`FileRef`] from a path. The path is *not* resolved or
    /// canonicalized — the sensor passed it in, and we trust it.
    #[must_use]
    pub fn new(path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            path: path.into(),
            mime: None,
        }
    }

    /// Attach a known MIME type.
    #[must_use]
    pub fn with_mime(mut self, mime: MimeType) -> Self {
        self.mime = Some(mime);
        self
    }
}

// --- Symbol (code) -----------------------------------------------------------

/// Reference to a code symbol — function, type, variable — within a file.
///
/// Used by IDE/LSP-style observers to expose AST-level handles.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SymbolRef {
    /// Owning file.
    pub file: FileRef,
    /// Fully-qualified symbol name (`module::SubModule::function`).
    pub qualified_name: String,
    /// Symbol kind ("function", "struct", "variable", ...). Free-form;
    /// observers stamp whatever the LSP told them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

impl SymbolRef {
    /// Construct from a file and qualified name. Rejects empty names.
    pub fn new(file: FileRef, qualified_name: impl Into<String>) -> Result<Self> {
        Ok(Self {
            file,
            qualified_name: validate_id("SymbolRef.qualified_name", &qualified_name.into())?,
            kind: None,
        })
    }

    /// Attach a kind.
    #[must_use]
    pub fn with_kind(mut self, kind: impl Into<String>) -> Self {
        self.kind = Some(kind.into());
        self
    }
}

// --- Text span / selection ---------------------------------------------------

/// A byte-offset span in a file or buffer.
///
/// `[start, end)` half-open. `end >= start` is enforced at construction. Spans
/// of length zero are allowed (pure cursor positions).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TextSpan {
    /// Byte offset of the span start.
    pub start: u64,
    /// Byte offset of the span end (exclusive).
    pub end: u64,
}

impl TextSpan {
    /// Construct a [`TextSpan`], rejecting `end < start`.
    pub fn new(start: u64, end: u64) -> Result<Self> {
        if end < start {
            return Err(SensoriumError::InvalidSpan { start, end });
        }
        Ok(Self { start, end })
    }

    /// Length of the span in bytes.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.end - self.start
    }

    /// `true` if the span has zero length (pure cursor position).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

/// Reference to a selection in a file — a `FileRef` plus a byte-range span.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SelectionRef {
    /// Owning file.
    pub file: FileRef,
    /// Byte span within the file.
    pub span: TextSpan,
}

impl SelectionRef {
    /// Construct from a file and span.
    #[must_use]
    pub fn new(file: FileRef, span: TextSpan) -> Self {
        Self { file, span }
    }
}

// --- URL ---------------------------------------------------------------------

/// A URL the substrate has observed (open browser tab, document link, etc.).
///
/// Stored as a string and not parsed — the substrate doesn't care about
/// schemes, only about preserving the value verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Uri(String);

impl Uri {
    /// Construct a [`Uri`], rejecting empty input.
    pub fn new(raw: impl Into<String>) -> Result<Self> {
        Ok(Self(validate_id("Uri", &raw.into())?))
    }

    /// Borrow the underlying URI string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
