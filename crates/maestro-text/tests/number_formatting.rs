//! Recorded finite-number presentation through the public formatter.
#![cfg(test)]
#![cfg(feature = "number-format")]
use maestro_text::numbers::LocaleNumberFormat;
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};

/// One recorded constructor and its output sequence.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    input: Input,
    expected: Vec<String>,
}
/// Supplied environment and decimal operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    environment: BTreeMap<String, String>,
    operands: Vec<String>,
}
#[test]
fn locale_numbers_match_recorded_grouping_and_rounding() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/number_formatting.json")).unwrap();
    let mut queries = HashSet::new();
    for case in cases {
        let numbers =
            LocaleNumberFormat::new(&|key| case.input.environment.get(key).cloned()).unwrap();
        assert_eq!(case.input.operands.len(), case.expected.len());
        for (operand, expected) in case.input.operands.iter().zip(case.expected) {
            assert!(queries.insert((
                serde_json::to_string(&case.input.environment).unwrap(),
                operand.clone()
            )));
            assert_eq!(numbers.format(operand.parse().unwrap()).unwrap(), expected);
        }
    }
    assert_eq!(queries.len(), 357);
}

#[test]
#[cfg(feature = "collation")]
fn locale_number_selection_is_captured_and_shared_with_collation() {
    use maestro_text::collation::Collator;
    use std::{cell::RefCell, cmp::Ordering};
    let environment = RefCell::new(BTreeMap::new());
    let asked = RefCell::new(Vec::new());
    let reader = |key: &str| {
        asked.borrow_mut().push(key.to_owned());
        environment.borrow().get(key).cloned()
    };
    for (entries, expected_calls, expected) in [
        (vec![], vec!["LC_ALL", "LC_MESSAGES", "LANG"], "1,234.5"),
        (
            vec![("LANG", "de_DE")],
            vec!["LC_ALL", "LC_MESSAGES", "LANG"],
            "1.234,5",
        ),
        (
            vec![("LC_MESSAGES", "fr_FR"), ("LANG", "de_DE")],
            vec!["LC_ALL", "LC_MESSAGES"],
            "1\u{202f}234,5",
        ),
        (
            vec![("LC_ALL", ""), ("LANG", "de_DE")],
            vec!["LC_ALL"],
            "1,234.5",
        ),
    ] {
        *environment.borrow_mut() = entries
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect();
        asked.borrow_mut().clear();
        let formatter = LocaleNumberFormat::new(&reader).unwrap();
        assert_eq!(*asked.borrow(), expected_calls);
        asked.borrow_mut().clear();
        let collator = Collator::new(&reader).unwrap();
        assert_eq!(*asked.borrow(), expected_calls);
        assert_eq!(collator.compare("z", "ä"), Ordering::Greater);
        environment.borrow_mut().clear();
        assert_eq!(formatter.format(1234.5).unwrap(), expected);
    }
    *environment.borrow_mut() = BTreeMap::from([("LC_ALL".into(), "de_DE".into())]);
    let german = LocaleNumberFormat::new(&reader).unwrap();
    *environment.borrow_mut() = BTreeMap::from([("LC_ALL".into(), "en_US".into())]);
    let english = LocaleNumberFormat::new(&reader).unwrap();
    assert_eq!(german.format(1234.5).unwrap(), "1.234,5");
    assert_eq!(english.format(1234.5).unwrap(), "1,234.5");
    assert!(english.format(f64::INFINITY).is_err());
    assert!(english.format(f64::NAN).is_err());
}
