//! Consumption of the recorded factory corpus without source metadata.
#![cfg(test)]
use super::{execution, factories, select};
use crate::read_support::Fixture;
use crate::runtime;
use crate::{loop_support, read_support};
use maestro_tools::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::HashSet, sync::Arc};

/// The consumed option states and queries.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    options: Vec<Value>,
    cases: Vec<Row>,
}
/// One observable query with its independent expectation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    id: String,
    op: String,
    input: Value,
    expected: Value,
}
/// Parse the complete corpus, rejecting unused top-level fields and repeated queries.
fn corpus() -> Corpus {
    let corpus: Corpus =
        serde_json::from_str(include_str!("../fixtures/tool-factory-cases.json")).unwrap();
    let mut seen = HashSet::new();
    for row in &corpus.cases {
        assert!(
            seen.insert((
                &row.op,
                row.input.to_string(),
                if row.op == "helper" {
                    row.id.as_str()
                } else {
                    ""
                }
            )),
            "repeated factory query"
        );
    }
    corpus
}

/// Names and complete helper records through both public export paths.
pub fn check_order() {
    let corpus = corpus();
    let manager = Arc::new(ToolsManager::new("/unused/search".into(), None));
    for row in corpus.cases {
        match row.op.as_str() {
            "enumeration" => assert_eq!(
                serde_json::to_value(ALL_TOOL_NAMES.map(|n| n.to_string())).unwrap(),
                row.expected
            ),
            "helper" => check_helper(&row),
            "routing"
                if row.expected.get("error").is_some()
                    || row.input.get("options") == Some(&json!(0))
                        && row.input.get("name").is_none() =>
            {
                check_routing_row(&row, &corpus.options, &manager);
            }
            _ => {}
        }
    }
    for name in ALL_TOOL_NAMES {
        let tool = create_tool_definition::<()>(
            &name.to_string(),
            "/missing".into(),
            ToolsOptions::default(),
            "/unused/bash".into(),
            manager.clone(),
        )
        .unwrap();
        assert_eq!(super::wrap(tool, None).definition.name, name.to_string());
    }
    let definitions = create_all_tool_definitions::<()>(
        "/missing".into(),
        ToolsOptions::default(),
        "/unused/bash".into(),
        manager,
    );
    assert_eq!(
        definitions.keys().copied().collect::<Vec<_>>(),
        ALL_TOOL_NAMES
    );
    let mut custom = create_read_tool("/missing".into(), ReadToolOptions::default());
    custom.definition.name = "caller-defined".into();
    assert_eq!(custom.definition.name, "caller-defined");
}

/// Execute the selected nondefault routing rows and fresh-construction observations.
pub fn check_routes() {
    let corpus = corpus();
    let manager = Arc::new(ToolsManager::new("/unused/search".into(), None));
    for row in corpus.cases {
        match row.op.as_str() {
            "routing"
                if row.expected.get("error").is_none()
                    && !(row.input["options"] == 0 && row.input.get("name").is_none()) =>
            {
                check_routing_row(&row, &corpus.options, &manager);
                let state = &corpus.options[row.input["options"]
                    .as_u64()
                    .map(|n| usize::try_from(n).unwrap())
                    .unwrap()];
                execution::paired(
                    row.input["factory"].as_str().unwrap(),
                    row.input["name"].as_str().unwrap_or(""),
                    row.input["cwd"].as_str().unwrap(),
                    state,
                );
            }
            "fresh" => check_fresh(&row, &manager),
            _ => {}
        }
    }
    image_options_and_context();
}

/// Constructor order and error paths, preserving caller spelling.
fn check_routing_row(row: &Row, states: &[Value], manager: &Arc<ToolsManager>) {
    let probes = execution::Probes::new(false);
    let state = &states[row.input["options"]
        .as_u64()
        .map(|n| usize::try_from(n).unwrap())
        .unwrap()];
    let before = probes.logs();
    let result = select(
        row.input["factory"].as_str().unwrap(),
        row.input["name"].as_str().unwrap_or("read"),
        row.input["cwd"].as_str().unwrap().into(),
        probes.options(state),
        manager.clone(),
    );
    assert_eq!(probes.logs(), before, "constructor had effects: {}", row.id);
    match result {
        Err(error) => {
            assert_eq!(json!(error.message), row.expected["error"]);
        }
        Ok(tools) => {
            let names: Vec<_> = tools.iter().map(|t| t.definition.name.as_str()).collect();
            assert_eq!(json!(names), row.expected["names"], "names: {}", row.id);
        }
    }
}

/// Separate constructions own separate execution closures; entries and containers are owned values.
fn check_fresh(row: &Row, manager: &Arc<ToolsManager>) {
    let factory = row.input["factory"].as_str().unwrap();
    let a = select(
        factory,
        "read",
        "/missing".into(),
        ToolsOptions::default(),
        manager.clone(),
    )
    .unwrap();
    let b = select(
        factory,
        "read",
        "/missing".into(),
        ToolsOptions::default(),
        manager.clone(),
    )
    .unwrap();
    assert_eq!(
        json!(a.iter().map(|t| &t.definition.name).collect::<Vec<_>>()),
        row.expected["names"]
    );
    assert_eq!(a.len(), b.len());
    assert_eq!(row.expected["executionFresh"], true);
    for (a, b) in a.iter().zip(&b) {
        assert!(!Arc::ptr_eq(&a.execute, &b.execute));
    }
}

/// Shared helper outputs retain their complete records, not only rendered text.
fn check_helper(row: &Row) {
    let options = TruncationOptions {
        max_lines: row.input["options"]["maxLines"]
            .as_u64()
            .map(|n| usize::try_from(n).unwrap()),
        max_bytes: None,
    };
    let text = row.input["text"].as_str().unwrap_or("");
    let both = match row.id.as_str() {
        "constants" => [
            json!({"DEFAULT_MAX_LINES":DEFAULT_MAX_LINES,"DEFAULT_MAX_BYTES":DEFAULT_MAX_BYTES}),
            json!({"DEFAULT_MAX_LINES":registry::DEFAULT_MAX_LINES,"DEFAULT_MAX_BYTES":registry::DEFAULT_MAX_BYTES}),
        ],
        "truncate-head" => [
            serde_json::to_value(truncate_head(text, options)).unwrap(),
            serde_json::to_value(registry::truncate_head(text, options)).unwrap(),
        ],
        "truncate-tail" => [
            serde_json::to_value(truncate_tail(text, options)).unwrap(),
            serde_json::to_value(registry::truncate_tail(text, options)).unwrap(),
        ],
        "truncate-line" => {
            let limit = usize::try_from(row.input["limit"].as_u64().unwrap()).unwrap();
            [
                truncate_line(text, Some(limit)),
                registry::truncate_line(text, Some(limit)),
            ]
            .map(|r| json!({"text":r.text,"wasTruncated":r.was_truncated}))
        }
        "format-size" => [
            json!(format_size(row.input["bytes"].as_f64().unwrap())),
            json!(registry::format_size(row.input["bytes"].as_f64().unwrap())),
        ],
        _ => panic!("unknown helper"),
    };
    for actual in both {
        assert_eq!(actual, row.expected);
    }
}

/// Full authored definition metadata, callback attachment and wrapper declarations.
pub fn check_metadata() {
    for row in corpus().cases.into_iter().filter(|r| r.op == "metadata") {
        let name = row.input["name"].as_str().unwrap();
        let manager = Arc::new(ToolsManager::new("/unused/search".into(), None));
        if row.input["kind"] == "wrapped" {
            let tool = create_tool(
                name,
                "/missing".into(),
                ToolsOptions::default(),
                "/unused/bash".into(),
                manager,
            )
            .unwrap();
            assert_eq!(
                json!({"name":tool.definition.name,"label":tool.label,"description":tool.definition.description,"parameters":tool.definition.parameters}),
                row.expected["metadata"]
            );
            assert_eq!(
                json!({"prepareArguments":tool.prepare_arguments.is_some(),"executionMode":tool.execution_mode,"renderCall":false,"renderResult":false}),
                row.expected["callbacks"]
            );
        } else {
            let definition = create_tool_definition::<()>(
                name,
                "/missing".into(),
                ToolsOptions::default(),
                "/unused/bash".into(),
                manager,
            )
            .unwrap();
            let actual = match definition {
                ToolDef::Read(t) => metadata(&t),
                ToolDef::Bash(t) => metadata(&t),
                ToolDef::Edit(t) => metadata(&t),
                ToolDef::Write(t) => metadata(&t),
                ToolDef::Grep(t) => metadata(&t),
                ToolDef::Find(t) => metadata(&t),
                ToolDef::Ls(t) => metadata(&t),
            };
            assert_eq!(
                actual["metadata"], row.expected["metadata"],
                "metadata {name}"
            );
            assert_eq!(
                actual["callbacks"]["prepareArguments"],
                row.expected["callbacks"]["prepareArguments"]
            );
            assert_eq!(
                actual["callbacks"]["executionMode"],
                row.expected["callbacks"]["executionMode"]
            );
            for key in ["renderCall", "renderResult"] {
                assert_eq!(row.expected["callbacks"][key], true);
                assert_eq!(actual["callbacks"][key], true);
            }
        }
    }
}

/// Project only exported definition metadata, preserving optional-field presence.
fn metadata<C, P, D, S>(tool: &ToolDefinition<C, P, D, S>) -> Value {
    let mut data = json!({"name":tool.definition.name,"label":tool.label,"description":tool.definition.description,"parameters":tool.definition.parameters});
    if let Some(snippet) = &tool.prompt_snippet {
        data["promptSnippet"] = json!(snippet);
    }
    if let Some(guidelines) = &tool.prompt_guidelines {
        data["promptGuidelines"] = json!(guidelines);
    }
    if let Some(shell) = &tool.render_shell {
        data["renderShell"] = json!(match shell {
            ToolRenderShell::SelfRendered => "self",
            ToolRenderShell::Default => "default",
        });
    }
    json!({"metadata":data,"callbacks":{"prepareArguments":tool.prepare_arguments.is_some(),"executionMode":tool.execution_mode,"renderCall":tool.render_call.is_some(),"renderResult":tool.render_result.is_some()}})
}

/// The owning agent loop prepares before validation and retains in-place argument mutations.
pub fn check_preparation() {
    for row in corpus().cases.into_iter().filter(|r| r.op == "prepare") {
        for factory in factories("edit") {
            let root = read_support::Scratch::new().unwrap();
            root.file("f", b"a c");
            let manager = execution::manager(&root);
            let tools = select(
                factory,
                "edit",
                root.text(),
                ToolsOptions::default(),
                manager,
            )
            .unwrap();
            let mut raw = row.input.clone();
            let edit = tools.iter().find(|t| t.definition.name == "edit").unwrap();
            assert_eq!(
                edit.prepare_arguments.as_ref().unwrap()(&mut raw).unwrap(),
                row.expected
            );
            let turn = loop_support::run_turn(tools, &[("edit-id", "edit", row.input.clone())]);
            assert_eq!(turn.arguments, vec![raw.clone()]);
            assert_eq!(turn.request_arguments, vec![raw]);
            assert_eq!(turn.results.len(), 1);
            let valid = row.expected["edits"]
                .as_array()
                .is_some_and(|a| !a.is_empty());
            assert_eq!(turn.results[0]["isError"], !valid);
            assert_eq!(
                std::fs::read(root.path().join("f")).unwrap(),
                if !valid {
                    b"a c"
                } else if row.expected["edits"].as_array().unwrap().len() == 2 {
                    b"b d"
                } else {
                    b"b c"
                }
            );
            if !valid {
                let individual = create_edit_tool(root.text(), EditToolOptions::default());
                let original = loop_support::run_turn(
                    vec![individual],
                    &[("edit-id", "edit", row.input.clone())],
                );
                compare_loop_results(&turn.results, &original.results);
            }
        }
    }
}

/// Ignore only wall-clock timestamps from separately completed invocations.
fn compare_loop_results(actual: &[Value], expected: &[Value]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        for key in [
            "role",
            "toolCallId",
            "toolName",
            "content",
            "details",
            "isError",
        ] {
            assert_eq!(actual[key], expected[key]);
        }
        assert!(actual["timestamp"].is_number() && expected["timestamp"].is_number());
    }
}

/// Read-image policy remains observable while an unrelated bash prefix varies.
fn image_options_and_context() {
    let image = read_support::blank_image(2200, 20, image::ImageOutputFormat::Png);
    for resize in [false, true] {
        for prefix in ["", "distinct-prefix"] {
            for factory in factories("read") {
                let root = read_support::Scratch::new().unwrap();
                let tool = select(
                    factory,
                    "read",
                    root.text(),
                    image_options(&image, resize, prefix),
                    execution::manager(&root),
                )
                .unwrap()
                .into_iter()
                .find(|tool| tool.definition.name == "read")
                .unwrap();
                assert_image(&tool, &image, resize, &maestro_models::ModelInput::Image);
            }
            image_context(&image, resize, prefix);
        }
    }
}

/// Supplied image operations with independently selected resize and shell-prefix options.
fn image_options(image: &[u8], resize: bool, prefix: &str) -> ToolsOptions {
    let ops = Arc::new(read_support::ScriptedOps::new(image).with_mime("image/png"));
    ToolsOptions {
        read: ReadToolOptions {
            auto_resize_images: Some(resize),
            operations: Some(ops),
        },
        bash: BashToolOptions {
            command_prefix: Some(prefix.into()),
            ..BashToolOptions::default()
        },
        ..ToolsOptions::default()
    }
}

/// Resize-sensitive image output with fresh contexts from each execution.
fn image_context(image: &[u8], resize: bool, prefix: &str) {
    use maestro_models::ModelInput;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let root = read_support::Scratch::new().unwrap();
    let options = image_options(image, resize, prefix);
    let definition = create_tool_definition::<read_support::SharedModel>(
        "read",
        "/work".into(),
        options,
        "/unused/bash".into(),
        execution::manager(&root),
    )
    .unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let calls = count.clone();
    let context: ToolContextFactory<_> = Arc::new(move || {
        let inputs = if calls.fetch_add(1, Ordering::SeqCst) == 0 {
            vec![ModelInput::Image]
        } else {
            vec![ModelInput::Text]
        };
        Ok(read_support::SharedModel(Arc::new(std::sync::RwLock::new(
            Some(read_support::model(inputs)),
        ))))
    });
    let tool = super::wrap(definition, Some(context));
    for supports_image in [true, false] {
        assert_image(
            &tool,
            image,
            resize,
            &if supports_image {
                ModelInput::Image
            } else {
                ModelInput::Text
            },
        );
    }
    assert_eq!(count.load(Ordering::SeqCst), 2);
}

/// Observe the image bytes and model-support note after the context is consumed.
fn assert_image(tool: &Tool, image: &[u8], resize: bool, model_input: &maestro_models::ModelInput) {
    use base64::Engine as _;
    use maestro_models::{ModelInput, UserBlock};
    let result = runtime::run((tool.execute)(
        "call".into(),
        json!({"path":"image.png"}),
        None,
        None,
    ))
    .unwrap();
    let text = serde_json::to_value(&result.content).unwrap();
    assert_eq!(
        text[0]["text"]
            .as_str()
            .unwrap()
            .contains("does not support images"),
        *model_input != ModelInput::Image
    );
    let UserBlock::Image(actual) = result.content.last().unwrap() else {
        panic!("missing image");
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&actual.data)
        .unwrap();
    assert_eq!(bytes == image, !resize);
}
