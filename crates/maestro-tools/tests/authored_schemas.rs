//! Bundled tool declarations and the schema nodes their authors declared.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use std::collections::BTreeMap;
use std::sync::Arc;

use maestro_models::{ParameterSchema, SchemaKind};
use maestro_tools::{
    BashToolOptions, EditToolOptions, FindToolOptions, GrepToolOptions, LsToolOptions,
    ReadToolOptions, ToolsManager, WriteToolOptions,
};
use serde::Deserialize;
use serde_json::Value;

/// One recorded producer case of the shared corpus.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Case label, matching the bundled tool name.
    #[serde(rename = "case")]
    label: String,
    /// Recorded group; only producer rows are read here.
    group: String,
    /// Supplied schema value.
    schema: Value,
    /// Declared node kinds by JSON pointer.
    kinds: BTreeMap<String, SchemaKind>,
    /// Recorded original arguments.
    arguments: serde_json::Map<String, Value>,
    /// Recorded prepared arguments or diagnostic.
    expected: Value,
}

/// Read the recorded producer rows.
fn productions() -> Vec<Case> {
    let cases: Vec<Case> = serde_json::from_str(include_str!(
        "../../maestro-models/tests/fixtures/tool_argument_conversion.json"
    ))
    .expect("recorded conversion corpus");
    cases
        .into_iter()
        .filter(|case| case.group == "producer")
        .collect()
}

/// The recorded producer case for one tool name.
fn recorded(name: &str) -> Case {
    let mut cases = productions()
        .into_iter()
        .filter(|case| case.label == name)
        .collect::<Vec<_>>();
    assert_eq!(cases.len(), 1, "one recorded producer case for {name}");
    let case = cases.pop().expect("recorded case");
    assert!(!case.schema.is_null() && !case.kinds.is_empty());
    assert!(!case.arguments.is_empty());
    case
}

/// Assert one declaration keeps the recorded JSON, node kinds and prepared outcome.
#[track_caller]
fn assert_declaration<C: maestro_models::SchemaCodeSource + Clone + Default>(
    name: &str,
    parameters: &ParameterSchema<C>,
) {
    let case = recorded(name);
    assert_eq!(parameters.as_json(), &case.schema, "{name} schema");
    assert_eq!(
        parameters.conversions().len(),
        case.kinds.len(),
        "{name} kinds"
    );
    for (pointer, kind) in &case.kinds {
        assert_eq!(
            parameters.conversions().get(pointer),
            Some(kind),
            "{name} kind at {pointer:?}"
        );
    }
    let tool = maestro_models::Tool {
        name: name.to_owned(),
        description: String::new(),
        parameters: parameters.clone(),
    };
    let call = maestro_models::ToolCall {
        id: "call".to_owned(),
        name: name.to_owned(),
        arguments: Value::Object(case.arguments.clone()),
        thought_signature: None,
    };
    let outcome = match maestro_models::validate_tool_arguments(&tool, &call) {
        Ok(arguments) => serde_json::json!({"Ok": arguments}),
        Err(error) => serde_json::json!({"Err": error.message}),
    };
    assert_eq!(outcome, case.expected, "{name} prepared outcome");
}

/// A manager that searches no directory.
fn manager() -> Arc<ToolsManager> {
    Arc::new(ToolsManager::new("/unused/search".into(), None))
}

#[test]
fn bundled_tool_schemas_keep_authored_nodes() {
    let cwd = "/work".to_owned();

    let read =
        maestro_tools::create_read_tool_definition::<()>(cwd.clone(), ReadToolOptions::default());
    assert_declaration("read", &read.definition.parameters);
    let write =
        maestro_tools::create_write_tool_definition::<()>(cwd.clone(), WriteToolOptions::default());
    assert_declaration("write", &write.definition.parameters);
    let edit =
        maestro_tools::create_edit_tool_definition::<()>(cwd.clone(), EditToolOptions::default());
    assert_declaration("edit", &edit.definition.parameters);
    let bash = maestro_tools::create_bash_tool_definition::<()>(
        cwd.clone(),
        String::new(),
        BashToolOptions::default(),
    );
    assert_declaration("bash", &bash.definition.parameters);
    let grep = maestro_tools::create_grep_tool_definition::<()>(
        cwd.clone(),
        GrepToolOptions::default(),
        manager(),
    );
    assert_declaration("grep", &grep.definition.parameters);
    let find = maestro_tools::create_find_tool_definition::<()>(
        cwd.clone(),
        FindToolOptions::default(),
        manager(),
    );
    assert_declaration("find", &find.definition.parameters);
    let ls = maestro_tools::create_ls_tool_definition::<()>(cwd.clone(), LsToolOptions::default());
    assert_declaration("ls", &ls.definition.parameters);

    let local_read = maestro_tools::read::local::create_read_tool_definition::<()>(
        cwd.clone(),
        ReadToolOptions::default(),
    );
    assert_declaration("read", &local_read.definition.parameters);
    let local_edit = maestro_tools::edit::local::create_edit_tool_definition::<()>(
        cwd.clone(),
        EditToolOptions::default(),
    );
    assert_declaration("edit", &local_edit.definition.parameters);
    let local_write = maestro_tools::write::local::create_write_tool_definition::<()>(
        cwd.clone(),
        WriteToolOptions::default(),
    );
    assert_declaration("write", &local_write.definition.parameters);
    let local_bash = maestro_tools::process::bash::local::create_bash_tool_definition::<()>(
        cwd,
        String::new(),
        maestro_tools::process::bash::local::BashToolOptions::default(),
    );
    assert_declaration("bash", &local_bash.definition.parameters);
}
