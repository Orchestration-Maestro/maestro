//! Timestamp conversion cases.

use super::timestamp;

#[test]
fn timestamp_preserves_utc_milliseconds() {
    let cases = [
        (0, "1970-01-01T00:00:00.000Z"),
        (1, "1970-01-01T00:00:00.001Z"),
        (999, "1970-01-01T00:00:00.999Z"),
        (1000, "1970-01-01T00:00:01.000Z"),
        (951_782_400_006, "2000-02-29T00:00:00.006Z"),
        (1_709_251_199_999, "2024-02-29T23:59:59.999Z"),
        (1_709_251_200_000, "2024-03-01T00:00:00.000Z"),
        (1_767_323_045_006, "2026-01-02T03:04:05.006Z"),
    ];
    for (millis, expected) in cases {
        assert_eq!(timestamp(millis), expected, "{millis} ms");
    }
}
