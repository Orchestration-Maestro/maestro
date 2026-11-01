//! Strict source-observation corpus and the renderer-only public driver.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::display_support::{Env, Setup, context};
use maestro_models::UserBlock;
use maestro_tools::{
    ToolRenderResultOptions, ToolRendererState, ToolsManager, ToolsOptions, create_tool_definition,
    output::render_utils::ToolRenderResultLike,
};
use serde::{Deserialize, Deserializer};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, HashSet},
    sync::Arc,
};

/// Present null is distinct from an omitted details member.
fn present<'de, D: Deserializer<'de>>(decoder: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(decoder).map(Some)
}
/// Inputs consumed by the public callback.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Input {
    /// Factory name.
    pub tool: String,
    /// Only read/edit results inspect args.
    pub args: Option<Value>,
    /// Persisted details, including present null.
    #[serde(default, deserialize_with = "present")]
    pub details: Option<Value>,
    /// Independent result flags.
    pub options: Options,
    /// Host error flag.
    pub is_error: bool,
    /// Ordered result content.
    pub content: Vec<UserBlock>,
}
/// Callback flags in the corpus.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Options {
    /// Expanded phase.
    pub expanded: bool,
    /// Streaming flag.
    pub is_partial: bool,
}
/// Independent ordered output rows.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expected {
    /// Exact rendered bytes.
    pub rows: Vec<String>,
}
/// One invocation and its observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// Callback inputs.
    pub input: Input,
    /// Oracle rows.
    pub expected: Expected,
}
/// All source observations plus the palette they use.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Every used foreground color.
    palette: BTreeMap<String, u8>,
    /// Ordered unique invocations.
    cases: Vec<Case>,
}
/// Build the actual factory binding and invoke it without execution.
pub fn render(input: &Input, env: &Env) -> Vec<String> {
    let tool = create_tool_definition::<()>(
        &input.tool,
        "/work".into(),
        ToolsOptions::default(),
        "/unused/bash".into(),
        Arc::new(ToolsManager::new("/unused/search".into(), None)),
    )
    .unwrap();
    let binding = tool.renderer_binding().unwrap();
    let args = input.args.as_ref().unwrap_or(&Value::Null);
    let mut row = context::<ToolRendererState>(
        args,
        &Setup::from(&json!({"readme":"/opt/maestro/README.md","bindings":[],"showImages":false})),
        env,
        None,
    );
    row.args = input.args.as_ref();
    row.is_error = input.is_error;
    let result = ToolRenderResultLike {
        content: &input.content,
        details: input.details.as_ref(),
        terminate: None,
        is_error: Some(input.is_error),
    };
    binding
        .render_result
        .unwrap()
        .prepare(result)
        .unwrap()
        .render(
            ToolRenderResultOptions {
                expanded: input.options.expanded,
                is_partial: input.options.is_partial,
            },
            &env.live,
            &row,
        )
        .unwrap()
        .render(80)
}
/// Replay a nonoverlapping source group, rejecting duplicate queries and unused palette values.
pub fn check(range: std::ops::Range<usize>) {
    let text = include_str!("../fixtures/renderer-projection.json");
    let corpus: Corpus = serde_json::from_str(text).unwrap();
    let raw: Value = serde_json::from_str(text).unwrap();
    let mut unique = HashSet::new();
    for case in raw["cases"].as_array().unwrap() {
        assert!(unique.insert(case["input"].to_string()), "duplicate query");
    }
    assert_eq!(corpus.cases.len(), 126);
    for color in corpus.palette.values() {
        let prefix = format!("\u{1b}[38;5;{color}m");
        assert!(
            corpus.cases.iter().any(|case| case
                .expected
                .rows
                .iter()
                .any(|row| row.contains(&prefix)))
        );
    }
    let env = environment();
    for (index, case) in corpus
        .cases
        .iter()
        .enumerate()
        .take(range.end)
        .skip(range.start)
    {
        assert_eq!(
            render(&case.input, &env),
            case.expected.rows,
            "query {index}"
        );
    }
}
use std::rc::Rc;

/// Custom definition whose executable callback must never be retained or called by a binding.
pub fn definition<S, D>() -> maestro_tools::types::local::ToolDefinition<(), (), D, S> {
    maestro_tools::types::local::ToolDefinition {
        definition: maestro_models::Tool {
            name: "custom".into(),
            description: String::new(),
            parameters: json!({}).into(),
        },
        label: "custom".into(),
        prompt_snippet: None,
        prompt_guidelines: None,
        render_shell: Some(maestro_tools::ToolRenderShell::SelfRendered),
        prepare_arguments: None,
        execution_mode: None,
        execute: Rc::new(|_, (), _, _, _| {
            Box::pin(async { panic!("renderer binding must not execute") })
        }),
        render_call: None,
        render_result: None,
    }
}

/// All observed result colors plus unobserved empty-style validations.
pub fn environment() -> Env {
    let corpus: Corpus =
        serde_json::from_str(include_str!("../fixtures/renderer-projection.json")).unwrap();
    let mut colors = serde_json::to_value(&corpus.palette).unwrap();
    for key in ["muted", "dim"] {
        colors.as_object_mut().unwrap().insert(key.into(), json!(0));
    }
    Env::with(
        Rc::new(crate::edit_render_support::theme(&colors, &json!({}))),
        Rc::new(crate::display_support::NoLanguages),
    )
}
