//! Calibration honesty: every observed value declares its calibration; the
//! substrate doesn't lie or default to "trusted".
//!
//! Properties under test:
//!
//! - **§9.4** "The model never determines whether to dispatch" — the
//!   contract decides. The substrate's job is to *honestly report*
//!   calibration, then let Pneuma apply policy.
//! - **§10.2** "Uncalibrated scores get a 20% effective penalty against
//!   thresholds." (We test the *signal*; the penalty itself lives in
//!   `pneuma-core`.)

use sensorium_core::{
    AppId, Calibration, CalibrationStatus, PrimitiveKind, PrivacyTier, Provenance, SensorId,
    Tagged, Timestamp,
};

/// `Calibration::calibrated_now()` is trusted; all four other constructors
/// are not. The default is *not trusted* — sensors must opt in.
#[test]
fn only_calibrated_status_is_trusted() {
    assert!(Calibration::calibrated_now().is_trusted());
    assert!(!Calibration::uncalibrated().is_trusted());
    assert!(!Calibration::synthetic().is_trusted());
    assert!(!Calibration::failed().is_trusted());
}

/// Substrate is_usable is broader than is_trusted — `Synthetic` and
/// `Uncalibrated` are usable (Pneuma may apply a penalty); only `Failed`
/// is not.
#[test]
fn only_failed_status_is_unusable() {
    assert!(CalibrationStatus::Calibrated.is_usable());
    assert!(CalibrationStatus::Uncalibrated.is_usable());
    assert!(CalibrationStatus::Synthetic.is_usable());
    assert!(!CalibrationStatus::Failed.is_usable());
}

/// `Provenance::is_calibrated` reflects the calibration discriminant.
/// This is the field Pneuma reads, so its semantics must match the
/// `Calibration::is_trusted` axis.
#[test]
fn provenance_calibration_matches_calibration_status() {
    let sensor = SensorId::new();
    let now = Timestamp::now();

    let calibrated = Provenance::new(
        sensor,
        now,
        Calibration::calibrated_now(),
        PrivacyTier::Public,
        PrimitiveKind::Reference,
    );
    let uncalibrated = Provenance::new(
        sensor,
        now,
        Calibration::uncalibrated(),
        PrivacyTier::Public,
        PrimitiveKind::Reference,
    );

    assert!(calibrated.is_calibrated());
    assert!(!uncalibrated.is_calibrated());
}

/// `Tagged<T>::is_calibrated` is a passthrough to the provenance's
/// calibration. Tested explicitly because consumers rely on it for
/// pattern-match arms.
#[test]
fn tagged_is_calibrated_is_provenance_passthrough() {
    let sensor = SensorId::new();
    let app = AppId::new("com.test.example").expect("non-empty id");

    let provenance = Provenance::new(
        sensor,
        Timestamp::now(),
        Calibration::uncalibrated(),
        PrivacyTier::Public,
        PrimitiveKind::Reference,
    );
    let tagged = Tagged::new(app, provenance);

    assert!(!tagged.is_calibrated());
    assert_eq!(tagged.value.as_str(), "com.test.example");
}

/// `Tagged::map` preserves provenance. Refining a value (e.g. resolving
/// a gaze point to a window) does not add evidence — the tag still
/// points at the original sensor.
#[test]
fn tagged_map_preserves_provenance_pointer() {
    let sensor = SensorId::new();
    let provenance = Provenance::new(
        sensor,
        Timestamp::now(),
        Calibration::synthetic(),
        PrivacyTier::Public,
        PrimitiveKind::Reference,
    );
    let original = Tagged::new(42_u32, provenance);

    let mapped = original.map(|n| n * 2);

    assert_eq!(mapped.value, 84);
    assert_eq!(mapped.provenance.sensor, sensor);
    assert_eq!(
        mapped.provenance.calibration.status,
        CalibrationStatus::Synthetic
    );
}

/// Aggregating calibration status across a stream of observations: if
/// *any* value is uncalibrated, the aggregate is uncalibrated. This is
/// the conservative composition rule — Pneuma applies it implicitly.
#[test]
fn calibration_aggregation_is_conservative() {
    let sensor = SensorId::new();
    let now = Timestamp::now();
    let mk = |cal: Calibration| -> Provenance {
        Provenance::new(
            sensor,
            now,
            cal,
            PrivacyTier::Public,
            PrimitiveKind::Reference,
        )
    };

    let stream: Vec<Provenance> = vec![
        mk(Calibration::calibrated_now()),
        mk(Calibration::calibrated_now()),
        mk(Calibration::uncalibrated()),
    ];

    let all_calibrated = stream.iter().all(Provenance::is_calibrated);
    assert!(
        !all_calibrated,
        "any uncalibrated value taints the aggregate"
    );
}

/// `Calibration::calibrated_now()` stamps a `last_calibrated_at`. The
/// staleness signal must be reachable by Pneuma to scale the penalty.
#[test]
fn calibrated_now_records_timestamp() {
    let cal = Calibration::calibrated_now();
    assert_eq!(cal.status, CalibrationStatus::Calibrated);
    assert!(
        cal.last_calibrated_at.is_some(),
        "calibrated_now must record when calibration happened"
    );
}

/// `Calibration::uncalibrated()` has no timestamp by design — there is
/// nothing to record.
#[test]
fn uncalibrated_has_no_timestamp() {
    let cal = Calibration::uncalibrated();
    assert_eq!(cal.status, CalibrationStatus::Uncalibrated);
    assert_eq!(cal.last_calibrated_at, None);
}
