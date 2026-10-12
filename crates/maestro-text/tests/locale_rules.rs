//! Locale preparation against recorded orders; requires the `collation` feature.
#![cfg(test)]
#![cfg(feature = "collation")]
use maestro_text::collation::Collator;
use serde::Deserialize;
use std::{cell::RefCell, cmp::Ordering, collections::HashMap};

/// One recorded locale environment and its results.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Locale {
    environment: HashMap<String, String>,
    ordered: Vec<String>,
    pairs: [i8; 4],
}

/// All recorded locale results.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rules {
    #[serde(rename = "localeNames")]
    names: Vec<String>,
    locale: Vec<Locale>,
}

/// Build a collator from `environment`.
fn collator(environment: &HashMap<String, String>) -> Collator {
    Collator::new(&|name| environment.get(name).cloned()).unwrap()
}

/// Build a collator from literal pairs.
fn from_pairs(pairs: &[(&str, &str)]) -> Collator {
    collator(
        &pairs
            .iter()
            .map(|(k, v)| ((*k).into(), (*v).into()))
            .collect(),
    )
}

/// The sign of an ordering as recorded.
fn sign(ordering: Ordering) -> i8 {
    ordering as i8
}

#[test]
fn locale_comparisons_match_recorded_orders() {
    let rules: Rules = serde_json::from_str(include_str!("fixtures/locale_rules.json")).unwrap();
    for case in &rules.locale {
        let collator = collator(&case.environment);
        let mut names = rules.names.clone();
        names.sort_by(|a, b| collator.compare(a, b));
        assert_eq!(names, case.ordered, "{:?}", case.environment);
        let pairs = [("z", "ä"), ("a", "A"), ("é", "e\u{301}"), ("I", "ı")];
        let observed = pairs.map(|(l, r)| sign(collator.compare(l, r)));
        assert_eq!(observed, case.pairs, "{:?}", case.environment);
    }
}

#[test]
fn locale_lookup_stops_at_first_present_operand() {
    let asked = RefCell::new(Vec::new());
    let observe = |env: &'static [(&'static str, &'static str)]| {
        asked.borrow_mut().clear();
        let collator = Collator::new(&|name| {
            asked.borrow_mut().push(name.to_owned());
            env.iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| (*v).to_owned())
        })
        .unwrap();
        (collator.compare("z", "ä"), asked.borrow().clone())
    };
    let all = &[
        ("LC_ALL", "de_DE"),
        ("LC_MESSAGES", "sv_SE"),
        ("LANG", "sv_SE"),
    ];
    assert_eq!(observe(all), (Ordering::Greater, vec!["LC_ALL".to_owned()]));
    let messages = &[("LC_MESSAGES", "sv_SE"), ("LANG", "de_DE")];
    let names = vec!["LC_ALL".to_owned(), "LC_MESSAGES".to_owned()];
    assert_eq!(observe(messages), (Ordering::Less, names));
    let empty_all = &[("LC_ALL", ""), ("LANG", "sv_SE")];
    let (order, asked_names) = observe(empty_all);
    assert_eq!(
        (order, asked_names),
        (Ordering::Greater, vec!["LC_ALL".to_owned()])
    );
    let language = &[("LANG", "sv_SE")];
    let names = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .map(str::to_owned)
        .to_vec();
    assert_eq!(observe(language), (Ordering::Less, names));
}

#[test]
fn collators_capture_independent_locale_choices() {
    let mut environment: HashMap<String, String> = HashMap::new();
    environment.insert("LANG".into(), "sv_SE".into());
    let swedish = collator(&environment);
    environment.insert("LANG".into(), "de_DE".into());
    let german = collator(&environment);
    environment.clear();
    drop(environment);
    assert_eq!(swedish.compare("z", "ä"), Ordering::Less);
    assert_eq!(german.compare("z", "ä"), Ordering::Greater);
    assert_eq!(from_pairs(&[]).compare("a", "A"), Ordering::Less);
}
