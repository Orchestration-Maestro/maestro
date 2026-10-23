//! Recorded model selection through shared public handles.
use maestro_catalog::{
    DEFAULT_MODEL_PER_PROVIDER, ModelRegistry, ParseModelPatternOptions, ResolveCliModelOptions,
    find_exact_model_reference_match, model_id_comparator, parse_model_pattern, resolve_cli_model,
};
use maestro_credentials::{AuthStorage, AuthStorageData};
use maestro_models::{Model, SharedHandle};
use serde::Deserialize;
use std::sync::{Arc, RwLock};
/// Shared descriptor identity.
pub type Handle = Arc<RwLock<Model>>;
/// Recorded authored input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Input {
    op: String,
    pattern: Option<String>,
    #[serde(default, deserialize_with = "super::descriptors::descriptors")]
    models: Vec<Model>,
    options: Option<Options>,
    cli_provider: Option<String>,
    cli_model: Option<String>,
    edits: Option<indexmap::IndexMap<String, String>>,
}
/// Strict source option record.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Options {
    allow_invalid_thinking_level_fallback: Option<bool>,
}
/// Complete expected public result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    #[serde(deserialize_with = "optional_descriptor")]
    model: Option<Model>,
    model_index: Option<usize>,
    thinking_level: Option<maestro_models::ModelThinkingLevel>,
    warning: Option<String>,
    error: Option<String>,
}
/// Selection or ordered table expectation.
#[derive(Deserialize)]
#[serde(untagged)]
enum Observation {
    Selection(Box<Expected>),
    Defaults(Vec<(String, String)>),
}
/// One independently recorded operation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    input: Input,
    expected: Observation,
}
/// Decode a complete optional fixture descriptor with no unread fields.
fn optional_descriptor<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> Result<Option<Model>, D::Error> {
    use serde::de::IntoDeserializer;
    let value = Option::<serde_json::Value>::deserialize(decoder)?;
    value
        .map(|value| {
            super::descriptors::descriptors(
                serde_json::Value::Array(vec![value]).into_deserializer(),
            )
            .map_err(serde::de::Error::custom)
            .map(|mut models| models.remove(0))
        })
        .transpose()
}
/// Construct the real registry over controlled shared descriptors.
pub fn registry(handles: &[Handle]) -> ModelRegistry {
    let registry = ModelRegistry::in_memory(SharedHandle::new(AuthStorage::in_memory(
        AuthStorageData::new(),
    )))
    .unwrap();
    *registry.get_all().write().unwrap() = handles.to_vec();
    registry
}
/// Complete recorded selected fields, without a new public serializer.
struct ResultFields {
    /// Selected descriptor.
    model: Option<Handle>,
    /// Explicit thinking.
    thinking: Option<maestro_models::ModelThinkingLevel>,
    /// Returned warning.
    warning: Option<String>,
    /// Returned error text.
    error: Option<String>,
}
/// Execute only the cases owned by the named witness.
pub fn replay(ids: &[&str]) {
    let cases = cases();
    for id in ids {
        let case = cases.iter().find(|c| c.id == *id).unwrap();
        if case.input.op == "defaults" {
            let Observation::Defaults(expected) = &case.expected else {
                panic!("table expectation")
            };
            let actual: Vec<_> = DEFAULT_MODEL_PER_PROVIDER
                .read()
                .unwrap()
                .iter()
                .map(|(p, m)| (p.clone(), m.clone()))
                .collect();
            assert_eq!(&actual, expected);
            continue;
        }
        if let Some(edits) = &case.input.edits {
            for (provider, id) in edits {
                DEFAULT_MODEL_PER_PROVIDER
                    .write()
                    .unwrap()
                    .insert(provider.clone(), id.clone());
            }
        }
        let handles: Vec<_> = case
            .input
            .models
            .iter()
            .cloned()
            .map(|m| Arc::new(RwLock::new(m)))
            .collect();
        let Observation::Selection(expected) = &case.expected else {
            panic!("selection expectation")
        };
        let result = execute(&case.input, &handles);
        check(result.model.as_ref(), expected, &handles, id);
        assert_eq!(
            result.thinking, expected.thinking_level,
            "thinking case {id}"
        );
        assert_eq!(result.warning, expected.warning, "warning case {id}");
        assert_eq!(result.error, expected.error, "error case {id}");
    }
}
/// Dispatch a fixture's actual operation and supplied options.
fn execute(input: &Input, handles: &[Handle]) -> ResultFields {
    let calls = std::cell::Cell::new(0);
    let native = model_id_comparator(&|_| None).unwrap();
    let compare = |left: &str, right: &str| {
        calls.set(calls.get() + 1);
        if input.pattern.as_deref() == Some("shared-name") {
            left.cmp(right)
        } else {
            native(left, right)
        }
    };
    let (model, thinking, warning, error) = match input.op.as_str() {
        "exact" => (
            find_exact_model_reference_match(input.pattern.as_deref().unwrap(), handles),
            None,
            None,
            None,
        ),
        "parse" => {
            let result = parse_model_pattern(
                input.pattern.as_deref().unwrap(),
                handles,
                ParseModelPatternOptions {
                    allow_invalid_thinking_level_fallback: input
                        .options
                        .as_ref()
                        .and_then(|o| o.allow_invalid_thinking_level_fallback)
                        .unwrap_or(true),
                },
                &compare,
            );
            (result.model, result.thinking_level, result.warning, None)
        }
        "cli" | "default-edit" => execute_cli(input, handles, &compare),
        _ => panic!("unknown recorded operation"),
    };
    if input.pattern.as_deref() == Some("shared-name") {
        assert!(calls.get() > 0);
    }
    ResultFields {
        model,
        thinking,
        warning,
        error,
    }
}
/// Compare complete descriptors, key order and returned identity.
fn check(selected: Option<&Handle>, expected: &Expected, handles: &[Handle], id: &str) {
    let value = selected
        .as_ref()
        .map(|h| serde_json::to_value(&*h.read().unwrap()).unwrap());
    let expected_value = expected
        .model
        .as_ref()
        .map(|v| serde_json::to_value(v).unwrap());
    assert!(value == expected_value, "descriptor case {id}");
    if let (Some(actual), Some(expected)) = (&value, &expected_value) {
        for field in ["headers", "compat"] {
            assert_eq!(
                object_orders(actual.get(field)),
                object_orders(expected.get(field)),
                "map sequence case {id}"
            );
        }
    }
    let index = selected
        .as_ref()
        .and_then(|h| handles.iter().position(|m| Arc::ptr_eq(h, m)));
    assert_eq!(index, expected.model_index, "identity case {id}");
}
/// Retain nested object key sequences without copying payload values.
fn object_orders(value: Option<&serde_json::Value>) -> Vec<Vec<&str>> {
    match value {
        Some(serde_json::Value::Object(object)) => {
            let mut result = vec![object.keys().map(String::as_str).collect()];
            for child in object.values() {
                result.extend(object_orders(Some(child)));
            }
            result
        }
        Some(serde_json::Value::Array(array)) => {
            array.iter().flat_map(|v| object_orders(Some(v))).collect()
        }
        _ => Vec::new(),
    }
}
/// Construct a controlled complete descriptor from the recorded template.
pub fn descriptor(id: &str, provider: &str) -> Handle {
    let cases = cases();
    let mut model = cases[0].input.models[0].clone();
    model.id = id.into();
    model.name = id.into();
    model.provider = provider.into();
    Arc::new(RwLock::new(model))
}

/// Parse recorded data once per isolated test process.
fn cases() -> &'static [Case] {
    static CASES: std::sync::LazyLock<Vec<Case>> = std::sync::LazyLock::new(|| {
        serde_json::from_str(include_str!("../fixtures/model_selection.json")).unwrap()
    });
    &CASES
}

/// Observe selection and the same registry's post-operation identity order.
fn execute_cli(
    input: &Input,
    handles: &[Handle],
    compare: &dyn Fn(&str, &str) -> std::cmp::Ordering,
) -> (
    Option<Handle>,
    Option<maestro_models::ModelThinkingLevel>,
    Option<String>,
    Option<String>,
) {
    let registry = registry(handles);
    let result = resolve_cli_model(
        ResolveCliModelOptions {
            cli_provider: input.cli_provider.as_deref(),
            cli_model: input.cli_model.as_deref(),
            model_registry: &registry,
        },
        compare,
    );
    let all = registry.get_all();
    let after = all.read().unwrap();
    assert_eq!(after.len(), handles.len());
    assert!(
        after.iter().zip(handles).all(|(a, b)| Arc::ptr_eq(a, b)),
        "catalog order changed"
    );
    (
        result.model,
        result.thinking_level,
        result.warning,
        result.error.map(|e| e.to_string()),
    )
}
