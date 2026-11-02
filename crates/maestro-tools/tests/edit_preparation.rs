//! Preparation through the edit definition's public callback.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::edit_observers;
use crate::edit_support;
use crate::loop_support;
use crate::read_support;
use edit_support::rows;
use maestro_tools::{EditToolOptions, create_edit_tool_definition};

#[test]
fn edit_preparation_preserves_nonobjects() {
    let tool = create_edit_tool_definition::<()>("/work".into(), EditToolOptions::default());
    for mut row in rows("edit_preparation_preserves_nonobjects") {
        assert_eq!(
            tool.prepare_arguments.as_ref().unwrap()(&mut row.input).unwrap(),
            row.expected
        );
    }
}

/// Compare preparation including surviving member order.
fn preparation(test: &str) {
    let tool = create_edit_tool_definition::<()>("/work".into(), EditToolOptions::default());
    for mut row in rows(test) {
        let observed = tool.prepare_arguments.as_ref().unwrap()(&mut row.input).unwrap();
        assert_eq!(observed, row.expected);
        if let (Some(observed), Some(expected)) = (observed.as_object(), row.expected.as_object()) {
            assert_eq!(
                observed.keys().collect::<Vec<_>>(),
                expected.keys().collect::<Vec<_>>()
            );
        }
    }
}

#[test]
fn edit_preparation_keeps_complete_arguments() {
    preparation("edit_preparation_keeps_complete_arguments");
}

#[test]
fn edit_preparation_appends_top_level_pair() {
    preparation("edit_preparation_appends_top_level_pair");
}

#[test]
fn edit_preparation_keeps_partial_pairs() {
    preparation("edit_preparation_keeps_partial_pairs");
}

#[test]
fn edit_preparation_parses_only_array_json() {
    preparation("edit_preparation_parses_only_array_json");
}

#[test]
fn edit_definition_keeps_schema_and_prompt_metadata() {
    use maestro_tools::{
        EditOperations, EditToolDetails, EditToolInput, ToolRenderShell, create_edit_tool,
    };
    let _: Option<std::sync::Arc<dyn EditOperations>> = None;
    let _: Option<EditToolInput> = None;
    let _: Option<maestro_tools::edit::EditToolInput> = None;
    let _: Option<std::sync::Arc<dyn maestro_tools::edit::EditOperations>> = None;
    let _: maestro_tools::edit::EditToolOptions = EditToolOptions::default();
    let _: maestro_tools::edit::EditToolDetails = EditToolDetails {
        diff: "x".into(),
        first_changed_line: None,
    };
    let _ = maestro_tools::edit::create_edit_tool_definition::<()>(
        "/work".into(),
        EditToolOptions::default(),
    );
    assert_eq!(
        create_edit_tool("/work".into(), EditToolOptions::default())
            .definition
            .name,
        "edit"
    );
    assert_eq!(
        maestro_tools::edit::create_edit_tool("/work".into(), EditToolOptions::default())
            .definition
            .name,
        "edit"
    );
    let tool = create_edit_tool_definition::<()>("/work".into(), EditToolOptions::default());
    let row = rows("edit_definition_keeps_schema_and_prompt_metadata").remove(0);
    assert_eq!(row.input, serde_json::json!({}));
    assert_eq!(tool.render_shell, Some(ToolRenderShell::SelfRendered));
    assert!(tool.render_call.is_some() && tool.render_result.is_some());
    assert_eq!(
        serde_json::json!({
            "name": tool.definition.name, "label": tool.label,
            "description": tool.definition.description, "parameters": tool.definition.parameters,
            "promptSnippet": tool.prompt_snippet, "promptGuidelines": tool.prompt_guidelines,
            "renderShell": "self", "hasPrepare": tool.prepare_arguments.is_some(), "hasMode": tool.execution_mode.is_some()
        }),
        row.expected
    );
    for (line, expected) in [
        (
            Some(3),
            serde_json::json!({"diff":"changed", "firstChangedLine":3}),
        ),
        (None, serde_json::json!({"diff":"changed"})),
    ] {
        let details = EditToolDetails {
            diff: "changed".into(),
            first_changed_line: line,
        };
        assert_eq!(serde_json::to_value(details).unwrap(), expected);
    }
}

#[test]
fn edit_arguments_use_preparation_before_shared_validation() {
    use maestro_models::arguments::validation::validate_schema;
    use maestro_models::{ToolCall, validate_tool_arguments};
    use serde::Deserialize;
    use serde_json::{Value, json};
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Expected {
        ok: bool,
        args: Option<Value>,
    }
    let tool = create_edit_tool_definition::<()>("/work".into(), EditToolOptions::default());
    for mut row in rows("edit_arguments_use_preparation_before_shared_validation") {
        let expected: Expected = serde_json::from_value(row.expected).unwrap();
        let args = tool.prepare_arguments.as_ref().unwrap()(&mut row.input).unwrap();
        match args {
            Value::Object(arguments) => {
                let call = ToolCall {
                    id: "id".into(),
                    name: "edit".into(),
                    arguments: arguments.into(),
                    thought_signature: None,
                };
                let observed = validate_tool_arguments(&tool.definition, &call);
                assert_eq!(observed.is_ok(), expected.ok);
                assert_eq!(observed.ok(), expected.args);
            }
            args => {
                assert!(!expected.ok && expected.args.is_none());
                assert!(
                    !validate_schema(&tool.definition.parameters, &args)
                        .unwrap()
                        .is_empty()
                );
            }
        }
    }
    // Authored null conversion belongs to the shared validator, not preparation.
    let call = ToolCall {
        id: "id".into(),
        name: "edit".into(),
        arguments: json!({"path":null,"edits":[{"oldText":null,"newText":null}]}),
        thought_signature: None,
    };
    assert_eq!(
        validate_tool_arguments(&tool.definition, &call).unwrap(),
        json!({"path":"null","edits":[{"oldText":"null","newText":"null"}]})
    );
}

#[test]
fn prepared_edit_runs_through_agent_loop() {
    use maestro_tools::create_edit_tool;
    use read_support::{Fixture, Scratch};
    use serde_json::json;
    for (id, args) in [
        (
            "legacy",
            json!({"path":"file","oldText":"hello","newText":"world"}),
        ),
        (
            "array",
            json!({"path":"file","edits":"[{\"oldText\":\"hello\",\"newText\":\"world\"}]"}),
        ),
        (
            "parse-fold",
            json!({"path":"file","edits":"[]","oldText":"hello","newText":"world"}),
        ),
    ] {
        let dir = Scratch::new().unwrap();
        let path = dir.file("file", b"hello");
        let mut retained = args.clone();
        if id == "array" {
            retained["edits"] = json!([{"oldText":"hello","newText":"world"}]);
        }
        if id == "parse-fold" {
            retained["edits"] = json!([]);
        }
        let turn = loop_support::run_turn(
            vec![create_edit_tool(dir.text(), EditToolOptions::default())],
            &[(id, "edit", args)],
        );
        assert_eq!(turn.arguments, vec![retained.clone()]);
        assert_eq!(turn.request_arguments, vec![retained]);
        assert_eq!(std::fs::read(path).unwrap(), b"world");
        assert_eq!(turn.updates, 0);
        assert_eq!(turn.results.len(), 1);
        assert_eq!(turn.results[0]["toolCallId"], id);
        assert_eq!(
            turn.results[0]["content"],
            json!([{"type":"text","text":"Successfully replaced 1 block(s) in file."}])
        );
        assert_eq!(
            turn.results[0]["details"],
            json!({"diff":"-1 hello\n+1 world","firstChangedLine":1})
        );
        assert_eq!(turn.results[0]["isError"], false);
    }
}

#[test]
fn sequential_edit_observers_share_normalized_call() {
    edit_observers::observe(maestro_agent::ToolExecutionMode::Sequential);
}

#[test]
fn parallel_edit_observers_share_normalized_call() {
    edit_observers::observe(maestro_agent::ToolExecutionMode::Parallel);
}

#[test]
fn parsed_edit_arguments_survive_validation_failure() {
    use maestro_tools::create_edit_tool;
    use read_support::{Fixture, Scratch};
    use serde_json::json;
    let dir = Scratch::new().unwrap();
    let path = dir.file("file", b"hello");
    let turn = loop_support::run_turn(
        vec![create_edit_tool(dir.text(), EditToolOptions::default())],
        &[(
            "bad",
            "edit",
            json!({"path":"file","edits":"[{\"oldText\":\"hello\"}]"}),
        )],
    );
    let retained = json!({"path":"file","edits":[{"oldText":"hello"}]});
    assert_eq!(turn.arguments, vec![retained.clone()]);
    assert_eq!(turn.request_arguments, vec![retained]);
    assert_eq!(turn.results[0]["isError"], true);
    assert_eq!(std::fs::read(path).unwrap(), b"hello");
    assert_eq!(turn.updates, 0);
}
