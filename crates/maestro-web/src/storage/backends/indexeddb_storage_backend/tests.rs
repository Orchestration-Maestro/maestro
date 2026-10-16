//! Source-recorded decisions used by the browser adapter.
use super::*;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;
use std::cell::RefCell;

/// Consumed policy rows.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Recorded inputs and observations.
    rows: Vec<Row>,
}
/// A single policy input and its expected result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    /// Case label used only on failure.
    id: String,
    /// Consuming test name.
    test: String,
    /// Decoded again into the selected policy's strict input type.
    input: Value,
    /// Source-derived observation.
    expected: Value,
}
/// Decode each selected policy input without ignoring additional fields.
fn cases<I: DeserializeOwned, E: DeserializeOwned>(name: &str) -> Vec<(String, I, E)> {
    decode_cases(
        include_str!("../../../../tests/fixtures/indexeddb_policy.json"),
        name,
    )
}
/// Decode strict rows shared by both policy recordings.
fn decode_cases<I: DeserializeOwned, E: DeserializeOwned>(
    source: &str,
    name: &str,
) -> Vec<(String, I, E)> {
    let corpus: Corpus = serde_json::from_str(source).unwrap();
    let cases: Vec<_> = corpus
        .rows
        .into_iter()
        .filter(|row| row.test == name)
        .map(|row| {
            (
                row.id,
                serde_json::from_value(row.input).unwrap(),
                serde_json::from_value(row.expected).unwrap(),
            )
        })
        .collect();
    assert!(!cases.is_empty());
    cases
}
/// Store-name observations before an upgrade.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SchemaInput {
    /// Ordered configured names.
    stores: Vec<String>,
    /// Names already present.
    existing: Vec<String>,
}
/// Construct the existing application-owned schema.
fn stores(names: Vec<String>) -> Vec<maestro_app::StoreConfig> {
    names
        .into_iter()
        .map(|name| maestro_app::StoreConfig {
            name,
            key_path: None,
            auto_increment: None,
            indices: None,
        })
        .collect()
}
#[test]
fn schema_creation_plan_preserves_order_and_existing_stores() {
    for (id, input, expected) in cases::<SchemaInput, Vec<String>>(
        "schema_creation_plan_preserves_order_and_existing_stores",
    ) {
        let config = stores(input.stores);
        let selected: Vec<_> =
            missing_stores(&config, |name| input.existing.iter().any(|n| n == name))
                .map(|store| store.name.clone())
                .collect();
        assert_eq!(selected, expected, "{id}");
    }
    let config = stores(vec!["z-last".into(), "a-first".into()]);
    assert_eq!(
        missing_stores(&config, |_| false)
            .map(|store| store.name.as_str())
            .collect::<Vec<_>>(),
        ["z-last", "a-first"]
    );
}
#[test]
fn schema_creation_plan_observes_prior_creations() {
    for (id, input, expected) in
        cases::<SchemaInput, Vec<String>>("schema_creation_plan_observes_prior_creations")
    {
        let config = stores(input.stores);
        let existing = RefCell::new(input.existing);
        let mut selected = Vec::new();
        for store in missing_stores(&config, |name| existing.borrow().iter().any(|n| n == name)) {
            selected.push(store.name.clone());
            existing.borrow_mut().push(store.name.clone());
        }
        assert_eq!(selected, expected, "{id}");
    }
}
/// Live key-path classification and explicit key.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PutInput {
    /// Truth of the live store's key path.
    inline: bool,
    /// Authored separate key.
    key: String,
}
#[test]
fn put_key_selection_uses_the_live_store_path() {
    for (id, input, expected) in
        cases::<PutInput, Option<String>>("put_key_selection_uses_the_live_store_path")
    {
        assert_eq!(
            explicit_key(input.inline, &input.key),
            expected.as_deref(),
            "{id}"
        );
    }
}
/// Name produced by a native exception.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ErrorInput {
    /// Platform exception name.
    name: String,
}
#[test]
fn browser_storage_error_categories_keep_failure_kinds() {
    for (id, input, expected) in
        cases::<ErrorInput, String>("browser_storage_error_categories_keep_failure_kinds")
    {
        assert_eq!(format!("{:?}", error_kind(&input.name)), expected, "{id}");
    }
    assert_eq!(error_kind("Error"), std::io::ErrorKind::Other);
    assert_eq!(error_kind("unrecognized"), std::io::ErrorKind::Other);
}

#[test]
fn final_request_owner_settles_before_detaching() {
    let events = std::rc::Rc::new(RefCell::new(Vec::new()));
    let settled = events.clone();
    let detached = events.clone();
    let owner = RequestTeardown {
        settle: Box::new(move || settled.borrow_mut().push("settle")),
        detach: Box::new(move || detached.borrow_mut().push("detach")),
    };
    drop(owner);
    assert_eq!(*events.borrow(), ["settle", "detach"]);
}

#[test]
fn retained_opening_waiter_drop_keeps_request_pending() {
    let events = std::rc::Rc::new(RefCell::new(Vec::new()));
    let settled = events.clone();
    let detached = events.clone();
    let opening = std::rc::Rc::new(RequestTeardown {
        settle: Box::new(move || settled.borrow_mut().push("settle")),
        detach: Box::new(move || detached.borrow_mut().push("detach")),
    });
    let waiter = opening.clone();
    drop(waiter);
    assert!(events.borrow().is_empty());
    drop(opening);
    assert_eq!(*events.borrow(), ["settle", "detach"]);
}

/// Decode the independently recorded adapter cases.
fn adapter_cases<I: DeserializeOwned, E: DeserializeOwned>(name: &str) -> Vec<(String, I, E)> {
    decode_cases(
        include_str!("../../../../tests/fixtures/indexeddb_adapter_policy.json"),
        name,
    )
}
/// Authored optional prefix.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PrefixInput {
    /// Unmodified prefix.
    prefix: Option<String>,
}
/// Browser range arguments.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Bounds {
    /// Inclusive lower key.
    lower: String,
    /// Inclusive upper key.
    upper: String,
    /// Lower openness.
    lower_open: bool,
    /// Upper openness.
    upper_open: bool,
}
#[test]
fn prefix_bounds_keep_empty_and_inclusive_fence() {
    for (id, input, expected) in
        adapter_cases::<PrefixInput, Option<Bounds>>("prefix_bounds_keep_empty_and_inclusive_fence")
    {
        let actual = prefix_bounds(input.prefix.as_deref());
        assert_eq!(
            actual,
            expected
                .as_ref()
                .map(|b| (b.lower.as_str(), b.upper.clone())),
            "{id}"
        );
        if let Some(bounds) = expected {
            assert!(!bounds.lower_open && !bounds.upper_open, "{id}");
        }
    }
}

/// Requested index order.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DirectionInput {
    /// Serialized direction.
    direction: String,
}
#[test]
fn index_direction_selects_native_cursor_order() {
    use maestro_app::browser::storage::IndexDirection;
    assert_eq!(IndexDirection::default(), IndexDirection::Asc);
    for (id, input, expected) in
        adapter_cases::<DirectionInput, String>("index_direction_selects_native_cursor_order")
    {
        let direction = match input.direction.as_str() {
            "asc" => IndexDirection::Asc,
            "desc" => IndexDirection::Desc,
            _ => panic!("unknown direction"),
        };
        assert_eq!(cursor_direction(direction), expected, "{id}");
    }
}

/// Optional platform byte estimates.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QuotaInput {
    /// Optional used bytes.
    usage: Option<f64>,
    /// Optional available bytes.
    quota: Option<f64>,
}
/// Recorded quota result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QuotaExpected {
    /// Used bytes.
    usage: f64,
    /// Available bytes.
    quota: f64,
    /// Percentage used.
    percent: f64,
}
/// Compare every recorded numeric field without rounding.
fn check_quota(name: &str) {
    for (id, input, expected) in adapter_cases::<QuotaInput, QuotaExpected>(name) {
        let actual = quota_info(input.usage, input.quota);
        assert_eq!(actual.usage.to_bits(), expected.usage.to_bits(), "{id}");
        assert_eq!(actual.quota.to_bits(), expected.quota.to_bits(), "{id}");
        assert_eq!(actual.percent.to_bits(), expected.percent.to_bits(), "{id}");
    }
}
#[test]
fn quota_values_default_independently() {
    check_quota("quota_values_default_independently");
}

#[test]
fn quota_percent_keeps_division_before_scaling() {
    check_quota("quota_percent_keeps_division_before_scaling");
}
