//! Spreadsheet value formatting through the application interface.
#![cfg(test)]

use crate::ssf_support::date_code::DateObservation;
use maestro_app::browser::ssf::init_table;
use serde::Deserialize;
use std::collections::BTreeMap;

/// Decode a consumed fixture record.
fn fixture<T: serde::de::DeserializeOwned>(text: &str) -> T {
    serde_json::from_str(text).unwrap()
}

/// One built-in entry.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TableEntry {
    id: u32,
    format: String,
}

/// Initialization input and its complete expected table.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TableSeed {
    seed: BTreeMap<u32, String>,
    expected: BTreeMap<u32, String>,
}

#[test]
fn builtin_formats_overwrite_known_entries_and_keep_custom_entries() {
    let entries: Vec<TableEntry> = fixture(include_str!("fixtures/ssf/table.json"));
    let seeds: Vec<TableSeed> = fixture(include_str!("fixtures/ssf/tableSeeds.json"));
    for TableSeed { mut seed, expected } in seeds {
        let returned = init_table(&mut seed);
        for entry in &entries {
            assert_eq!(
                returned.get(&entry.id),
                Some(&entry.format),
                "id {}",
                entry.id
            );
        }
        assert_eq!(*returned, expected);
        returned.insert(400, "edited".into());
        assert_eq!(seed.get(&400).map(String::as_str), Some("edited"));
    }
}

/// An independent query of both default lookups.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DefaultQuery {
    id: u32,
    mapped: Option<u32>,
    format: Option<String>,
}

#[test]
fn default_formats_preserve_sparse_mapping_and_accounting_codes() {
    let queries: Vec<DefaultQuery> = fixture(include_str!("fixtures/ssf/defaultQueries.json"));
    for query in queries {
        assert_eq!(
            maestro_app::browser::ssf::default_map(query.id),
            query.mapped,
            "id {}",
            query.id
        );
        assert_eq!(
            maestro_app::browser::ssf::default_str(query.id),
            query.format.as_deref(),
            "id {}",
            query.id
        );
    }
}

/// Scalar ingress categories with only their consumed values.
#[derive(Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
enum ScalarCase {
    #[serde(rename = "missing")]
    Missing { expected: String },
    #[serde(rename = "null")]
    Null { expected: String },
    #[serde(rename = "string")]
    Text { value: String, expected: String },
    #[serde(rename = "boolean")]
    Boolean { value: bool, expected: String },
}

#[test]
fn general_scalars_preserve_text_booleans_and_empty_values() {
    use maestro_app::browser::ssf::{GeneralValue, general};
    let cases: Vec<ScalarCase> = fixture(include_str!("fixtures/ssf/scalars.json"));
    for case in cases {
        match case {
            ScalarCase::Missing { expected } | ScalarCase::Null { expected } => {
                assert_eq!(general(GeneralValue::Empty), expected);
            }
            ScalarCase::Text { value, expected } => {
                assert_eq!(general(GeneralValue::Text(&value)), expected);
            }
            ScalarCase::Boolean { value, expected } => {
                assert_eq!(general(GeneralValue::Boolean(value)), expected);
            }
        }
    }
}

/// Numeric spelling and General output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GeneralCase {
    value: String,
    expected: String,
}

#[test]
fn general_numbers_follow_width_precision_and_exponent_rules() {
    use maestro_app::browser::ssf::{GeneralValue, general};
    let cases: Vec<GeneralCase> = fixture(include_str!("fixtures/ssf/general.json"));
    for (index, case) in cases.into_iter().enumerate() {
        assert_eq!(
            general(GeneralValue::Number(case.value.parse().unwrap())),
            case.expected,
            "case {index}"
        );
    }
}

/// A bounded fraction query.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FractionCase {
    value: String,
    denominator: u32,
    mixed: bool,
    expected: [f64; 3],
}

#[test]
fn fractions_keep_best_convergents_and_signed_mixed_parts() {
    let cases: Vec<FractionCase> = fixture(include_str!("fixtures/ssf/fractions.json"));
    for (index, case) in cases.into_iter().enumerate() {
        assert_eq!(
            maestro_app::browser::ssf::frac(
                case.value.parse().unwrap(),
                case.denominator,
                case.mixed
            )
            .partial_cmp(&case.expected),
            Some(std::cmp::Ordering::Equal),
            "case {index}"
        );
    }
}

/// A serial query and every returned observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DateCase {
    value: String,
    #[serde(rename = "date1904")]
    date1904: bool,
    expected: Option<DateObservation>,
}

/// Mutually exclusive date corpus partitions.
#[derive(Clone, Copy, PartialEq)]
enum DatePartition {
    Invalid,
    Timezone,
    Integral,
    Fractional,
}

/// Classify each row once, before invoking the operation.
fn date_partition(case: &DateCase) -> DatePartition {
    let value: f64 = case.value.parse().unwrap();
    if case.expected.is_none() {
        DatePartition::Invalid
    } else if [40906.0, 40907.0, 40908.0].contains(&value) {
        DatePartition::Timezone
    } else if value.fract() == 0.0 {
        DatePartition::Integral
    } else {
        DatePartition::Fractional
    }
}

/// Compare all fields for the requested partition, including absence.
fn check_dates(partition: DatePartition) {
    use maestro_app::browser::ssf::{DateCode, parse_date_code};
    let cases: Vec<DateCase> = fixture(include_str!("fixtures/ssf/dates.json"));
    let mut checked = 0;
    for (index, case) in cases
        .into_iter()
        .enumerate()
        .filter(|(_, case)| date_partition(case) == partition)
    {
        let expected = case.expected.map(DateCode::from);
        assert_eq!(
            parse_date_code(case.value.parse().unwrap(), case.date1904),
            expected,
            "case {index}"
        );
        checked += 1;
    }
    assert!(checked > 0);
}

#[test]
fn date_codes_preserve_both_epochs_and_calendar_sentinels() {
    check_dates(DatePartition::Integral);
}

#[test]
fn date_codes_keep_subseconds_and_carry_consistent() {
    check_dates(DatePartition::Fractional);
}

#[test]
fn date_codes_reject_invalid_serials_before_epoch_shift() {
    check_dates(DatePartition::Invalid);
}

#[test]
fn date_codes_ignore_host_timezone_skipped_days() {
    check_dates(DatePartition::Timezone);
}
