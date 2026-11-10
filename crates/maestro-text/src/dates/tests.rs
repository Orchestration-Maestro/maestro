//! Committed recipe and index validation at the data-loading boundary.
use super::{load, recipe, representative};

/// One complete numeric recipe with all required fields.
const VALID: &str = "gregory\tlatn\th12\t%M/%d/%y, %H:%2m:%2s %p\tAM\tPM\t";
/// Root representatives required by fallback.
const INDEX: &str = "und\t0\nund-US\t0\nund-FR\t0\nund-IR\t0\nund-TH\t0\n";

#[test]
fn committed_date_time_data_rejects_malformed_records() {
    assert!(load(VALID, INDEX).is_ok());
    for record in [
        "",
        "gregory\tlatn\th12\t%y\tAM\tPM\t",
        "other\tlatn\th12\t%M/%d/%y %H:%m:%s %p\tAM\tPM\t",
        "gregory\tunknown\th12\t%M/%d/%y %H:%m:%s %p\tAM\tPM\t",
        "gregory\tlatn\th25\t%M/%d/%y %H:%m:%s %p\tAM\tPM\t",
        "gregory\tlatn\th12\t%M/%d/%y %H:%m:%s %p\t\tPM\t",
        "gregory\tlatn\th12\t%M/%d/%y %H:%m:%s %p\tAM\t\t",
        "gregory\tlatn\th23\t%M/%d/%y %H:%m:%s %e\t\t\t",
        "gregory\tlatn\th23\t%M/%d/%y %H:%m:%s %x\t\t\t",
        "gregory\tlatn\th23\t%M/%d/%y %H:%m:%s %y\t\t\t",
        "gregory\tlatn\th23\t%M/%d/%y %H:%m:%s %2p\tAM\tPM\t",
        "gregory\tlatn\th23\t%M/%d/%y %H:%m:%s %\t\t\t",
    ] {
        assert!(
            recipe(record).is_err(),
            "accepted malformed recipe: {record:?}"
        );
    }
    for index in [
        "",
        "und\t1\n",
        "und\tNaN\n",
        "und\t0\nund\t0\n",
        "not_a_locale\t0\n",
        "und\t0\n",
    ] {
        assert!(
            load(VALID, index).is_err(),
            "accepted malformed index: {index:?}"
        );
    }
}

/// Calendar rows refuse missing fields, unknown calendars and repeated regions.
#[test]
fn committed_date_time_calendars_reject_malformed_records() {
    let locale = "en-US".parse().unwrap();
    for calendars in ["US", "US\tgregory", "US\tpersian\nUS\tbuddhist"] {
        assert!(
            representative(&locale, calendars, "001\th23").is_err(),
            "accepted malformed calendars: {calendars:?}"
        );
    }
}

/// Hour rows refuse missing fields, unknown cycles, repeated regions and missing defaults.
#[test]
fn committed_date_time_hours_reject_malformed_records() {
    let locale = "en-US".parse().unwrap();
    for hours in ["US", "US\th25", "US\th12\nUS\th23", "FR\th23"] {
        assert!(
            representative(&locale, "", hours).is_err(),
            "accepted malformed hours: {hours:?}"
        );
    }
}
