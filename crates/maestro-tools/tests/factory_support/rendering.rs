//! Selected definitions retain the producing owner's typed render callbacks.
#![cfg(test)]
use crate::read_support::Fixture;
use crate::runtime;
use crate::{display_support, read_support};
use maestro_models::DiagnosticErrorInfo;
use maestro_tools::output::render_utils::ToolRenderResultLike;
use maestro_tools::*;
use serde_json::json;
use std::sync::Arc;

/// Compare the actual call/result components without erasing their state types.
fn compare<P, D: serde::Serialize, S>(
    selected: &ToolDefinition<(), P, D, S>,
    original: &ToolDefinition<(), P, D, S>,
) where
    S: Default,
{
    let args = super::execution::arguments(&selected.definition.name);
    let env = display_support::Env::with(
        display_support::shipped_theme("dark", None),
        display_support::native_highlighter(),
    );
    let recorded = json!({"readme":"/opt/maestro/README.md","bindings":["ctrl+o"],"expanded":true,"argsComplete":true});
    let setup = display_support::Setup::from(&recorded);
    let _bindings = display_support::bind(&display_support::expand_keys(&recorded));
    let draw = |tool: &ToolDefinition<(), P, D, S>| {
        let context = display_support::context::<S>(&args, &setup, &env, None);
        let call = (tool.render_call.as_ref().unwrap())(Some(&args), &env.live, &context).unwrap();
        let rows = call.render(80);
        let mut reused = display_support::context::<S>(&args, &setup, &env, Some(call.clone()));
        reused.state = context.state.clone();
        let next = (tool.render_call.as_ref().unwrap())(Some(&args), &env.live, &reused).unwrap();
        assert!(std::rc::Rc::ptr_eq(&call, &next));
        assert_eq!(next.render(80), rows);
        let result = maestro_agent::AgentToolResult::<D> {
            content: serde_json::from_value(json!([{"type":"text","text":"complete"}])).unwrap(),
            details: None,
            terminate: None,
            usage: None,
        };
        let component = crate::display_support::native(
            tool.render_result.as_ref().unwrap(),
            &ToolRenderResultLike::from(&result),
            ToolRenderResultOptions {
                expanded: true,
                is_partial: false,
            },
            &env.live,
            &context,
        )
        .unwrap();
        (rows, component.render(80))
    };
    assert_eq!(draw(selected), draw(original));
    let futures = env.runtime.take_futures();
    runtime::run(async {
        for future in futures {
            future.await.unwrap();
        }
    });
    assert_eq!(env.runtime.futures(), 0);
}

/// Compare each typed variant through every applicable definition preset.
pub fn check() {
    for factory in ["single", "coding", "readonly", "all"] {
        let root = read_support::Scratch::new().unwrap();
        let manager = Arc::new(ToolsManager::new(root.text(), None));
        let definitions = definitions(factory, &root, &manager);
        for definition in definitions {
            match definition {
                ToolDef::Read(t) => compare(
                    &t,
                    &create_read_tool_definition(root.text(), ReadToolOptions::default()),
                ),
                ToolDef::Bash(t) => compare(
                    &t,
                    &create_bash_tool_definition(
                        root.text(),
                        root.text(),
                        BashToolOptions::default(),
                    ),
                ),
                ToolDef::Edit(t) => compare(
                    &t,
                    &create_edit_tool_definition(root.text(), EditToolOptions::default()),
                ),
                ToolDef::Write(t) => compare(
                    &t,
                    &create_write_tool_definition(root.text(), WriteToolOptions::default()),
                ),
                ToolDef::Grep(t) => compare(
                    &t,
                    &create_grep_tool_definition(
                        root.text(),
                        GrepToolOptions::default(),
                        manager.clone(),
                    ),
                ),
                ToolDef::Find(t) => compare(
                    &t,
                    &create_find_tool_definition(
                        root.text(),
                        FindToolOptions::default(),
                        manager.clone(),
                    ),
                ),
                ToolDef::Ls(t) => compare(
                    &t,
                    &create_ls_tool_definition(root.text(), LsToolOptions::default()),
                ),
            }
        }
    }
    context_error();
}

/// Obtain each definition preset without conflating its typed rendering state.
fn definitions(
    factory: &str,
    root: &read_support::Scratch,
    manager: &Arc<ToolsManager>,
) -> Vec<ToolDef> {
    match factory {
        "single" => ALL_TOOL_NAMES
            .iter()
            .map(|name| {
                create_tool_definition::<()>(
                    &name.to_string(),
                    root.text(),
                    ToolsOptions::default(),
                    root.text(),
                    manager.clone(),
                )
                .unwrap()
            })
            .collect(),
        "coding" => create_coding_tool_definitions::<()>(
            root.text(),
            ToolsOptions::default(),
            root.text(),
            manager.clone(),
        ),
        "readonly" => create_read_only_tool_definitions::<()>(
            root.text(),
            ToolsOptions::default(),
            root.text(),
            manager.clone(),
        ),
        "all" => create_all_tool_definitions::<()>(
            root.text(),
            ToolsOptions::default(),
            root.text(),
            manager.clone(),
        )
        .into_values()
        .collect(),
        _ => unreachable!(),
    }
}

/// A context error precedes the selected read's supplied operations.
fn context_error() {
    let root = read_support::Scratch::new().unwrap();
    let probes = super::execution::Probes::new(false);
    let before = probes.logs();
    let definition = create_tool_definition::<()>(
        "read",
        "/work".into(),
        probes.options(&super::execution::all_options()),
        root.text(),
        super::execution::manager(&root),
    )
    .unwrap();
    let failure = DiagnosticErrorInfo::error("context rejected");
    let error = failure.clone();
    let tool = super::wrap(definition, Some(Arc::new(move || Err(error.clone()))));
    assert_eq!(
        super::execution::execute(&tool, json!({"path":"f"}), None),
        Err(failure)
    );
    assert_eq!(probes.logs(), before);
}
