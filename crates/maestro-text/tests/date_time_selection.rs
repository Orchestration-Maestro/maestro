//! Locale capture, fallback and calendar boundaries through supplied wall fields.
#![cfg(test)]
#![cfg(feature = "date-time-format")]
use maestro_text::date_time_format::LocaleDateTimeFormat;
use std::{cell::RefCell, collections::BTreeMap};

/// Recorded environment observations and their selected presentation.
#[derive(serde::Deserialize)]
struct Selection {
    /// First-present environment values.
    environment: BTreeMap<String, String>,
    /// Ordered keys consulted during preparation.
    calls: Vec<String>,
    /// Recorded output for the fixed local fields.
    expected: String,
}

#[test]
fn locale_date_times_preserve_parent_script_and_calendar_boundaries() {
    let cases: Vec<(String, [i32; 3], [u8; 3], String)> =
        serde_json::from_str(include_str!("fixtures/date_time_selection.json")).unwrap();
    for (locale, date, time, expected) in cases {
        let formatter =
            LocaleDateTimeFormat::new(&|key| (key == "LANG").then(|| locale.clone())).unwrap();
        assert_eq!(
            formatter.format(date, time).unwrap(),
            expected,
            "{locale} {date:?}"
        );
    }
}

#[test]
fn locale_date_times_capture_first_present_environment() {
    let cases: Vec<Selection> =
        serde_json::from_str(include_str!("fixtures/date_time_environment.json")).unwrap();
    for Selection {
        environment,
        calls: expected_calls,
        expected,
    } in cases
    {
        let environment = RefCell::new(environment);
        let calls = RefCell::new(Vec::new());
        let formatter = LocaleDateTimeFormat::new(&|key| {
            calls.borrow_mut().push(key.to_owned());
            environment.borrow().get(key).cloned()
        })
        .unwrap();
        assert_eq!(*calls.borrow(), expected_calls);
        environment
            .borrow_mut()
            .insert("LC_ALL".into(), "fa_IR".into());
        assert_eq!(
            formatter.format([2026, 10, 27], [9, 25, 7]).unwrap(),
            expected
        );
        assert_eq!(*calls.borrow(), expected_calls);
    }
}

#[test]
fn locale_date_times_reject_invalid_wall_fields() {
    let formatter = LocaleDateTimeFormat::new(&|_| None).unwrap();
    for date in [
        [2026, 2, 29],
        [2026, 0, 1],
        [2026, 13, 1],
        [2026, 1, 0],
        [2026, 4, 31],
    ] {
        assert!(formatter.format(date, [0, 0, 0]).is_err());
    }
    for time in [[24, 0, 0], [0, 60, 0], [0, 0, 60]] {
        assert!(formatter.format([2026, 1, 1], time).is_err());
    }
}
