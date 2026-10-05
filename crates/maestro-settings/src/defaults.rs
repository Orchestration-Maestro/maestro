use serde_json::{Map, Value, json};

pub(crate) fn defaults() -> Map<String, Value> {
    json!({
        "packages": [],
        "extensions": [],
        "skills": [],
        "promptTemplates": [],
        "enableSkillCommands": true,
        "quietStartup": false,
        "hideThinking": false,
        "collapseChangelog": false
    })
    .as_object()
    .expect("default settings are an object")
    .clone()
}
