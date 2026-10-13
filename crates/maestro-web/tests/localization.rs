//! Shared localization values through the public context.
#![cfg(test)]
mod support;
use indexmap::IndexMap;
use maestro_web::{I18n, TranslationValue};
use serde::Deserialize;
use std::rc::Rc;
use support::Host;

/// Captured catalog entry.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Message {
    /// Authored lookup key.
    key: String,
    /// Resolved message.
    expected: String,
}
/// Require a textual leaf.
fn text(value: TranslationValue) -> String {
    match value {
        TranslationValue::Text(s) => s,
        _ => panic!("expected text"),
    }
}
#[test]
fn localization_catalog_preserves_every_message() {
    let host = Rc::new(Host::default());
    let context = I18n::new(host.clone()).unwrap();
    let tables: IndexMap<String, Vec<Message>> =
        serde_json::from_str(include_str!("fixtures/tables.json")).unwrap();
    for (language, messages) in tables {
        *host.stored.borrow_mut() = Some(language);
        for message in messages {
            assert_eq!(
                text(context.i18n(&message.key, None).unwrap()),
                message.expected
            );
        }
    }
    let defaults: IndexMap<String, IndexMap<String, String>> =
        serde_json::from_str(include_str!("fixtures/defaults.json")).unwrap();
    for (language, messages) in defaults {
        let table = if language == "en" {
            context.default_english()
        } else {
            context.default_german()
        };
        assert_eq!(
            table.borrow().keys().collect::<Vec<_>>(),
            messages.keys().collect::<Vec<_>>()
        );
        for (key, expected) in messages {
            assert_eq!(text(table.borrow()[&key].clone()), expected);
        }
    }
}

/// Controlled language inputs.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct LanguageInput {
    /// Persisted spelling.
    stored: Option<String>,
    /// Browser locale.
    locale: Option<String>,
    /// Legacy locale.
    legacy: Option<String>,
    /// Failing host effect.
    fail: Option<String>,
}
/// Success or failure outcome.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Outcome {
    /// Successful text.
    value: Option<String>,
    /// Failure category.
    error: Option<String>,
}
/// Language decision and closed effect sequence.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LanguageCase {
    /// Host input.
    input: LanguageInput,
    /// Captured outcome.
    expected: Outcome,
    /// Effects that completed synchronously.
    effects: Vec<String>,
}
/// Replay one decision against a fresh context.
fn language_case(case: LanguageCase) {
    let host = Rc::new(Host::default());
    *host.stored.borrow_mut() = case.input.stored;
    *host.locale.borrow_mut() = case.input.locale;
    *host.legacy.borrow_mut() = case.input.legacy;
    *host.fail.borrow_mut() = case.input.fail.unwrap_or_default();
    let context: I18n = I18n::new(host.clone()).unwrap();
    let result = context.get_current_language();
    match (case.expected.value, case.expected.error) {
        (Some(value), None) => assert_eq!(result.unwrap(), value),
        (None, Some(error)) => assert_eq!(result.unwrap_err().to_string(), error),
        _ => panic!("invalid oracle outcome"),
    }
    assert_eq!(*host.effects.borrow(), case.effects);
}
#[test]
fn current_language_obeys_storage_and_locale_precedence() {
    let cases: Vec<LanguageCase> =
        serde_json::from_str(include_str!("fixtures/languages.json")).unwrap();
    for case in cases.into_iter().filter(|c| c.input.fail.is_none()) {
        language_case(case);
    }
}
#[test]
fn current_language_reads_only_needed_host_fields() {
    let cases: Vec<LanguageCase> =
        serde_json::from_str(include_str!("fixtures/languages.json")).unwrap();
    for case in cases.into_iter().filter(|c| c.input.fail.is_some()) {
        language_case(case);
    }
}
/// Language write input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectionInput {
    /// Exact code to persist.
    code: String,
    /// Failing effect.
    fail: String,
}
/// Retained state after selection.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectionOutcome {
    /// Persisted result.
    stored: String,
    /// Optional failure.
    error: Option<String>,
}
/// Write and reload witness.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectionCase {
    /// Operation input.
    input: SelectionInput,
    /// Retained result.
    expected: SelectionOutcome,
    /// Completed synchronous effects.
    effects: Vec<String>,
}
/// Observe the same operation's result, effects and storage.
fn selection_case(case: SelectionCase) {
    let host = Rc::new(Host::default());
    *host.stored.borrow_mut() = Some("en".into());
    *host.fail.borrow_mut() = case.input.fail;
    let context: I18n = I18n::new(host.clone()).unwrap();
    let result = context.set_language(&case.input.code);
    match case.expected.error {
        Some(error) => assert_eq!(result.unwrap_err().to_string(), error),
        None => result.unwrap(),
    }
    assert_eq!(*host.stored.borrow(), Some(case.expected.stored));
    assert_eq!(*host.effects.borrow(), case.effects);
}
#[test]
fn language_selection_writes_before_reloading() {
    let cases: Vec<SelectionCase> =
        serde_json::from_str(include_str!("fixtures/selection.json")).unwrap();
    for case in cases.into_iter().filter(|c| c.input.fail.is_empty()) {
        selection_case(case);
    }
}
#[test]
fn language_selection_propagates_write_and_reload_failures() {
    let cases: Vec<SelectionCase> =
        serde_json::from_str(include_str!("fixtures/selection.json")).unwrap();
    for case in cases.into_iter().filter(|c| !c.input.fail.is_empty()) {
        selection_case(case);
    }
}

/// Literal or nested fixture value.
#[derive(Deserialize)]
#[serde(untagged)]
enum FixtureValue {
    /// Literal text.
    Text(String),
    /// Nested messages.
    Category(IndexMap<String, Self>),
}
/// Wrap independently recorded messages in shared handles.
fn fixture_messages(values: IndexMap<String, FixtureValue>) -> maestro_web::I18nMessages {
    Rc::new(std::cell::RefCell::new(
        values
            .into_iter()
            .map(|(k, v)| {
                let value = match v {
                    FixtureValue::Text(s) => TranslationValue::Text(s),
                    FixtureValue::Category(c) => TranslationValue::Category(fixture_messages(c)),
                };
                (k, value)
            })
            .collect(),
    ))
}
/// Lookup invocation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LookupInput {
    /// Caller tables.
    tables: IndexMap<String, IndexMap<String, FixtureValue>>,
    /// Host selection.
    language: Option<String>,
    /// Flat or nested operands.
    args: Vec<String>,
}
/// Lookup observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LookupCase {
    /// Supplied state.
    input: LookupInput,
    /// Return or failure.
    expected: Outcome,
    /// Synchronous diagnostics.
    diagnostics: Vec<String>,
}
/// Exercise the public lookup and notification boundary.
fn lookup_case(case: LookupCase) {
    let host = Rc::new(Host::default());
    *host.stored.borrow_mut() = case.input.language;
    let mut context = I18n::new(host.clone()).unwrap();
    context.set_translations(Rc::new(std::cell::RefCell::new(
        case.input
            .tables
            .into_iter()
            .map(|(k, v)| (k, fixture_messages(v)))
            .collect(),
    )));
    let result = context.i18n(
        &case.input.args[0],
        case.input.args.get(1).map(String::as_str),
    );
    match (case.expected.value, case.expected.error) {
        (Some(value), None) => assert_eq!(text(result.unwrap()), value),
        (None, Some(error)) => {
            assert_eq!(error, "missing_language");
            assert!(matches!(
                result,
                Err(maestro_web::I18nError::MissingLanguage(_))
            ));
        }
        _ => panic!("invalid oracle outcome"),
    }
    let diagnostics: Vec<_> = host
        .diagnostics
        .borrow()
        .iter()
        .map(|d| match d {
            maestro_web::I18nDiagnostic::MissingKey {
                category: None,
                key,
            } => format!("Unknown i18n key: {key}"),
            maestro_web::I18nDiagnostic::MissingKey {
                category: Some(category),
                key,
            } => format!("Unknown i18n key: {category}.{key}"),
            maestro_web::I18nDiagnostic::MissingCategory { category } => {
                format!("Unknown i18n category: {category}")
            }
        })
        .collect();
    assert_eq!(diagnostics, case.diagnostics);
}
/// Select disjoint corpus witnesses by original recording index.
fn lookup_cases(indices: &[usize]) {
    let cases: Vec<LookupCase> =
        serde_json::from_str(include_str!("fixtures/lookups.json")).unwrap();
    for (index, case) in cases.into_iter().enumerate() {
        if indices.contains(&index) {
            lookup_case(case);
        }
    }
}
#[test]
fn localization_returns_key_for_missing_or_empty_messages() {
    lookup_cases(&[1, 2, 10]);
}
#[test]
fn localization_returns_nested_values_or_nested_key() {
    lookup_cases(&[3, 4, 5, 6, 7]);
}
#[test]
fn localization_uses_custom_languages_without_per_key_english_fallback() {
    lookup_cases(&[0, 8, 9]);
}

/// Make an empty shared dictionary.
fn empty_messages() -> maestro_web::I18nMessages {
    Rc::new(std::cell::RefCell::new(IndexMap::new()))
}
#[test]
fn translation_handles_survive_edits_and_replacement() {
    let host = Rc::new(Host::default());
    *host.stored.borrow_mut() = Some("en".into());
    let mut context: I18n = I18n::new(host).unwrap();
    let first = Rc::new(std::cell::RefCell::new(IndexMap::new()));
    let english = empty_messages();
    let nested = empty_messages();
    english
        .borrow_mut()
        .insert("word".into(), TranslationValue::Text("before".into()));
    english
        .borrow_mut()
        .insert("group".into(), TranslationValue::Category(nested.clone()));
    first.borrow_mut().insert("en".into(), english.clone());
    context.set_translations(first.clone());
    let held = context.get_translations();
    let TranslationValue::Category(category) = context.i18n("group", None).unwrap() else {
        panic!("expected category")
    };
    assert!(Rc::ptr_eq(&nested, &category));
    held.borrow()["en"]
        .borrow_mut()
        .insert("word".into(), TranslationValue::Text("edited".into()));
    category
        .borrow_mut()
        .insert("key".into(), TranslationValue::Text("after".into()));
    assert_eq!(text(context.i18n("word", None).unwrap()), "edited");
    assert!(Rc::ptr_eq(&first, &held));
    assert_eq!(text(context.i18n("group", Some("key")).unwrap()), "after");
    let guard = english.borrow_mut();
    assert!(matches!(
        context.i18n("word", None),
        Err(maestro_web::I18nError::BorrowConflict)
    ));
    drop(guard);
    let second_english = empty_messages();
    second_english
        .borrow_mut()
        .insert("word".into(), TranslationValue::Text("second".into()));
    let second = Rc::new(std::cell::RefCell::new(IndexMap::from([(
        "en".into(),
        second_english,
    )])));
    context.set_translations(second.clone());
    assert!(Rc::ptr_eq(&context.get_translations(), &second));
    assert_eq!(text(context.i18n("word", None).unwrap()), "second");
    assert_eq!(text(held.borrow()["en"].borrow()["word"].clone()), "edited");
    let weak = Rc::downgrade(&first);
    drop(first);
    assert!(weak.upgrade().is_some());
    drop(held);
    assert!(weak.upgrade().is_none());
}

/// Typed callable input unlike the default string arguments.
struct TokenRecord {
    /// Unit label.
    unit: String,
}
#[test]
fn localization_returns_callable_identity_without_invoking_it() {
    let host = Rc::new(Host::default());
    *host.stored.borrow_mut() = Some("en".into());
    let mut context: I18n = I18n::new(host.clone()).unwrap();
    let table = empty_messages();
    table
        .borrow_mut()
        .insert("word".into(), TranslationValue::Text("before".into()));
    let calls = Rc::new(std::cell::Cell::new(0));
    let callback: Rc<maestro_web::TranslationFunction> = {
        let captured = table.clone();
        let calls = calls.clone();
        Rc::new(move |args| {
            calls.set(calls.get() + 1);
            format!("{}, {}", text(captured.borrow()["word"].clone()), args[0])
        })
    };
    let nested = empty_messages();
    nested.borrow_mut().insert(
        "function".into(),
        TranslationValue::Function(callback.clone()),
    );
    table.borrow_mut().insert(
        "function".into(),
        TranslationValue::Function(callback.clone()),
    );
    table
        .borrow_mut()
        .insert("group".into(), TranslationValue::Category(nested.clone()));
    context.set_translations(Rc::new(std::cell::RefCell::new(IndexMap::from([(
        "en".into(),
        table.clone(),
    )]))));
    let TranslationValue::Function(flat) = context.i18n("function", None).unwrap() else {
        panic!("expected callable")
    };
    let TranslationValue::Function(inner) = context.i18n("group", Some("function")).unwrap() else {
        panic!("expected callable")
    };
    assert!(Rc::ptr_eq(&callback, &flat));
    assert!(Rc::ptr_eq(&callback, &inner));
    assert_eq!(calls.get(), 0);
    table
        .borrow_mut()
        .insert("word".into(), TranslationValue::Text("edited".into()));
    assert_eq!(flat(&["reader".into()]), "edited, reader");
    assert_eq!(calls.get(), 1);
    let weak = Rc::downgrade(&table);
    table.borrow_mut().shift_remove("function");
    nested.borrow_mut().shift_remove("function");
    drop((callback, flat, inner, context, table, nested));
    assert!(weak.upgrade().is_none());
    typed_callable(host);
}
/// Numeric/record signatures keep the same callback handle.
fn typed_callable(host: Rc<Host>) {
    type Callable = dyn Fn(f64, &TokenRecord) -> String;
    let mut context: I18n<Callable> = I18n::new(host).unwrap();
    let callback: Rc<Callable> = Rc::new(|n, record| format!("{n} {}", record.unit));
    let table = Rc::new(std::cell::RefCell::new(IndexMap::from([(
        "count".into(),
        TranslationValue::Function(callback.clone()),
    )])));
    context.set_translations(Rc::new(std::cell::RefCell::new(IndexMap::from([(
        "en".into(),
        table,
    )]))));
    let TranslationValue::Function(found) = context.i18n("count", None).unwrap() else {
        panic!("expected callable")
    };
    assert!(Rc::ptr_eq(&callback, &found));
    assert_eq!(
        found(
            3.0,
            &TokenRecord {
                unit: "tokens".into()
            }
        ),
        "3 tokens"
    );
}
#[test]
fn bootstrap_tables_keep_their_original_identity() {
    let host = Rc::new(Host::default());
    *host.stored.borrow_mut() = Some("en".into());
    let mut context: I18n = I18n::new(host).unwrap();
    let original = context.translations();
    assert!(Rc::ptr_eq(&original, &context.get_translations()));
    context
        .default_english()
        .borrow_mut()
        .insert("Copy".into(), TranslationValue::Text("changed".into()));
    assert_eq!(text(context.i18n("Copy", None).unwrap()), "Copy");
    let replacement = empty_messages();
    replacement
        .borrow_mut()
        .insert("Free".into(), TranslationValue::Text("replacement".into()));
    context.set_translations(Rc::new(std::cell::RefCell::new(IndexMap::from([(
        "en".into(),
        replacement,
    )]))));
    assert!(Rc::ptr_eq(&original, &context.translations()));
    assert_eq!(
        text(original.borrow()["en"].borrow()["Free"].clone()),
        "Free"
    );
    assert_eq!(text(context.i18n("Free", None).unwrap()), "replacement");
}
