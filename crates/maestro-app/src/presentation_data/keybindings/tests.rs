//! Complete application defaults observed through the ordered table.
use super::*;
use serde::Deserialize;
use serde_json::{Map, Value};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    input: Input,
    expected: Expected,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    platform: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    definitions: std::collections::BTreeMap<String, Definition>,
    order: Vec<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Definition {
    default_keys: Value,
    description: String,
}

#[test]
fn application_defaults_keep_complete_order_keys_and_descriptions() {
    let mut corpus: Map<String, Value> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/application_keys.json"
    ))
    .unwrap();
    let cases: Vec<Case> = serde_json::from_value(
        corpus
            .shift_remove("application_defaults_keep_complete_order_keys_and_descriptions")
            .unwrap(),
    )
    .unwrap();
    for case in cases {
        let definitions = application_definitions(case.input.platform == "windows");
        if (case.input.platform == "windows") == cfg!(windows) {
            assert_eq!(*KEYBINDINGS, definitions);
        }
        assert_eq!(
            definitions
                .iter()
                .map(|(id, _)| id.clone())
                .collect::<Vec<_>>(),
            case.expected.order
        );
        for (id, definition) in &definitions {
            let expected = &case.expected.definitions[id];
            let actual = match &definition.default_keys {
                KeybindingKeys::Single(key) => serde_json::json!(key),
                KeybindingKeys::Multiple(keys) => serde_json::json!(keys),
            };
            assert_eq!(actual, expected.default_keys, "{id}");
            assert_eq!(
                definition.description.as_deref(),
                Some(expected.description.as_str()),
                "{id}"
            );
        }
    }
}
