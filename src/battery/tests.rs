use super::*;

#[test]
fn decode_date_le_string() {
    // The gauge writes the code string "280423" (DDMMYY) little-endian,
    // so the integer's big-endian digits read "324082".
    let raw = i64::from_be_bytes([0, 0, b'3', b'2', b'4', b'0', b'8', b'2']);
    assert_eq!(decode_date(raw).as_deref(), Some("2023-04-28"));
}

#[test]
fn decode_date_rejects_garbage() {
    assert_eq!(decode_date(0), None);
    assert_eq!(decode_date(-1), None);
    assert_eq!(decode_date(i64::MAX), None);
}

#[test]
fn decode_date_rejects_codes_that_are_not_dates() {
    // "999999": month 99 is invalid under every layout.
    let raw = i64::from_be_bytes([0, 0, b'9', b'9', b'9', b'9', b'9', b'9']);
    assert_eq!(decode_date(raw), None);
}

#[test]
fn valid_date_checks_the_calendar() {
    assert!(is_valid_date(2024, 2, 29)); // leap year
    assert!(!is_valid_date(2023, 2, 29));
    assert!(!is_valid_date(2023, 2, 30));
    assert!(!is_valid_date(2023, 4, 31));
    assert!(!is_valid_date(2023, 13, 1));
    assert!(!is_valid_date(2004, 1, 1)); // outside the gauge's plausible range
}

#[test]
fn condition_skips_health_heuristic_when_design_capacity_unknown() {
    // Regression: a missing design capacity yields health 0.0, which used to
    // be misreported as "Service Recommended".
    assert_eq!(condition_from(false, 0, 0.0), Condition::Normal);
    assert_eq!(
        condition_from(false, 4381, 79.9),
        Condition::ServiceRecommended
    );
    assert_eq!(condition_from(false, 4381, 88.0), Condition::Normal);
    assert_eq!(
        condition_from(true, 4381, 100.0),
        Condition::ServiceRecommended
    );
}

#[test]
fn fmt_duration_shapes() {
    assert_eq!(fmt_duration(45), "45 min");
    assert_eq!(fmt_duration(120), "2 h");
    assert_eq!(fmt_duration(106), "1 h 46 min");
}
