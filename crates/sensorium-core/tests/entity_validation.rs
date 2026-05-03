//! Entity-type validation. The substrate refuses to round-trip empty IDs
//! and invalid spans rather than silently accepting them.
//!
//! Properties under test:
//!
//! - **§5.1 Reference channel**: "gaze fixation against workspace
//!   substrate, hit-test resolved." A hit-test against an empty
//!   identifier is a sensor bug; we'd rather fail fast.
//! - **§6.3 guarantee 2**: "No directive dispatches with mismatched
//!   referent types." The first line of defense is rejecting malformed
//!   referents at construction.

use sensorium_core::{
    AppId, FileRef, MimeType, SelectionRef, SensoriumError, SymbolRef, TextSpan, Uri, WindowId,
    WindowRect,
};

#[test]
fn empty_app_id_rejected() {
    let err = AppId::new("").expect_err("empty rejected");
    assert!(matches!(
        err,
        SensoriumError::EmptyIdentifier { field: "AppId" }
    ));
}

#[test]
fn whitespace_app_id_rejected() {
    let err = AppId::new("   \t\n").expect_err("whitespace rejected");
    assert!(matches!(
        err,
        SensoriumError::EmptyIdentifier { field: "AppId" }
    ));
}

#[test]
fn valid_app_id_constructs_and_trims() {
    let id = AppId::new("  com.example.app  ").expect("trims and accepts");
    assert_eq!(id.as_str(), "com.example.app");
}

#[test]
fn empty_window_id_rejected() {
    let err = WindowId::new("").expect_err("empty rejected");
    assert!(matches!(
        err,
        SensoriumError::EmptyIdentifier { field: "WindowId" }
    ));
}

#[test]
fn empty_uri_rejected() {
    let err = Uri::new("").expect_err("empty rejected");
    assert!(matches!(
        err,
        SensoriumError::EmptyIdentifier { field: "Uri" }
    ));
}

#[test]
fn mime_type_normalizes_case() {
    let m = MimeType::new("Text/Plain").expect("valid mime");
    assert_eq!(m.as_str(), "text/plain");
}

#[test]
fn empty_mime_type_rejected() {
    let err = MimeType::new("   ").expect_err("empty rejected");
    assert!(matches!(
        err,
        SensoriumError::EmptyIdentifier { field: "MimeType" }
    ));
}

#[test]
fn text_span_with_end_before_start_rejected() {
    let err = TextSpan::new(10, 5).expect_err("end < start rejected");
    assert!(matches!(
        err,
        SensoriumError::InvalidSpan { start: 10, end: 5 }
    ));
}

#[test]
fn text_span_zero_length_allowed_as_cursor() {
    let span = TextSpan::new(42, 42).expect("zero-length is a cursor position");
    assert_eq!(span.len(), 0);
    assert!(span.is_empty());
}

#[test]
fn empty_symbol_ref_qualified_name_rejected() {
    let err = SymbolRef::new(FileRef::new("/tmp/x.rs"), "").expect_err("empty rejected");
    assert!(matches!(
        err,
        SensoriumError::EmptyIdentifier {
            field: "SymbolRef.qualified_name"
        }
    ));
}

#[test]
fn selection_ref_round_trips_through_constructor() {
    let span = TextSpan::new(0, 100).expect("valid");
    let sel = SelectionRef::new(FileRef::new("/etc/hosts"), span);
    assert_eq!(sel.span.start, 0);
    assert_eq!(sel.span.end, 100);
    assert_eq!(sel.span.len(), 100);
}

#[test]
fn window_rect_contains_correctly() {
    let rect = WindowRect {
        x: 10,
        y: 20,
        width: 100,
        height: 50,
    };
    // Inside corners.
    assert!(rect.contains(10, 20));
    assert!(rect.contains(109, 69));
    // Just outside.
    assert!(!rect.contains(9, 20));
    assert!(!rect.contains(10, 19));
    assert!(!rect.contains(110, 20));
    assert!(!rect.contains(10, 70));
}

#[test]
fn window_rect_zero_size_contains_nothing() {
    // A zero-sized rect (e.g. a minimized window) hit-tests as empty.
    let rect = WindowRect {
        x: 5,
        y: 5,
        width: 0,
        height: 0,
    };
    assert!(!rect.contains(5, 5));
}

#[test]
fn file_ref_attaches_mime() {
    let f = FileRef::new("/etc/passwd").with_mime(MimeType::new("text/plain").unwrap());
    assert_eq!(f.mime.as_ref().map(MimeType::as_str), Some("text/plain"));
}
