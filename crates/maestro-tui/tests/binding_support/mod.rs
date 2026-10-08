#![cfg(test)]

use maestro_tui::{
    KeybindingDefinition, KeybindingKeys, KeybindingsConfig, KeybindingsManager, TUI_KEYBINDINGS,
};
use serde_json::{Value, json};

/// Loads independently recorded binding observations.
pub fn corpus() -> Value {
    serde_json::from_str(include_str!("../fixtures/binding_cases.json")).unwrap()
}
/// Converts a fixture scalar or list into the public typed form.
pub fn keys(value: &Value) -> KeybindingKeys {
    if let Some(key) = value.as_str() {
        KeybindingKeys::Single(key.to_owned())
    } else {
        KeybindingKeys::Multiple(
            value
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_owned())
                .collect(),
        )
    }
}
/// Converts ordered fixture pairs, retaining explicit unset values.
pub fn config(value: &Value) -> KeybindingsConfig {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|pair| {
            (
                pair[0].as_str().unwrap().to_owned(),
                (!pair[1].is_null()).then(|| keys(&pair[1])),
            )
        })
        .collect()
}
/// Constructs the fixture manager using only public operations.
pub fn manager(input: &Value) -> KeybindingsManager {
    let definitions = if input["definitions"] == "builtins" {
        TUI_KEYBINDINGS.clone()
    } else {
        input["definitions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|pair| {
                (
                    pair[0].as_str().unwrap().to_owned(),
                    KeybindingDefinition {
                        default_keys: keys(&pair[1]["defaultKeys"]),
                        description: pair[1]["description"].as_str().map(str::to_owned),
                    },
                )
            })
            .collect()
    };
    KeybindingsManager::new(definitions, config(&input["user"]))
}
/// Renders a typed observation for comparison with the recorded corpus.
pub fn key_value(keys: KeybindingKeys) -> Value {
    match keys {
        KeybindingKeys::Single(key) => json!(key),
        KeybindingKeys::Multiple(keys) => json!(keys),
    }
}
/// Observes the manager through the corpus-selected public queries.
pub fn observe(manager: &KeybindingsManager, expected: &Value) -> Value {
    let keys: Vec<_> = expected["keys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|pair| {
            let id = pair[0].as_str().unwrap();
            json!([id, manager.get_keys(id)])
        })
        .collect();
    let definitions: Vec<_> = expected["definitions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|pair| {
            let id = pair[0].as_str().unwrap();
            let definition = manager.get_definition(id).map(|d| {
                let mut value = json!({"defaultKeys": key_value(d.default_keys)});
                if let Some(description) = d.description {
                    value["description"] = json!(description);
                }
                value
            });
            json!([id, definition])
        })
        .collect();
    let pairs = |config: KeybindingsConfig| {
        config
            .into_iter()
            .map(|(id, keys)| json!([id, keys.map(key_value)]))
            .collect::<Vec<_>>()
    };
    let conflicts: Vec<_> = manager
        .get_conflicts()
        .into_iter()
        .map(|conflict| json!({"key": conflict.key, "keybindings": conflict.keybindings}))
        .collect();
    let matches: Vec<_> = expected["matches"].as_array().unwrap().iter().map(|probe|
        json!({"data": probe["data"], "id": probe["id"], "result": manager.matches(probe["data"].as_str().unwrap(), probe["id"].as_str().unwrap())})).collect();
    json!({"keys": keys, "definitions": definitions, "user": pairs(manager.get_user_bindings()),
        "resolved": pairs(manager.get_resolved_bindings()), "conflicts": conflicts, "matches": matches})
}
/// Checks every corpus input owned by the named behavior test.
pub fn run(test: &str) {
    let corpus = corpus();
    assert!(
        corpus["cases"]
            .as_array()
            .unwrap()
            .iter()
            .any(|case| case["test"] == test)
    );
    for case in corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["test"] == test)
    {
        let manager = manager(&case["input"]);
        assert_eq!(
            observe(&manager, &case["expected"]["before"]),
            case["expected"]["before"],
            "{} before",
            case["id"]
        );
        if let Some(replacement) = case["input"].get("replacement") {
            manager.set_user_bindings(config(replacement));
            assert_eq!(
                observe(&manager, &case["expected"]["after"]),
                case["expected"]["after"],
                "{} after",
                case["id"]
            );
        }
    }
}
