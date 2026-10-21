//! Real native edit reads composed with the public retained row and differential writer.
use crate::live;
use crate::recording_terminal;
use maestro_chat::{ToolExecutionComponent, ToolExecutionResult};
use maestro_tui::{Component, TUI, Text};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{io::Write, rc::Rc};
/// One source-native scenario, including source-computed settled diff details.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    test: String,
    kind: String,
    input: Value,
    expected: Expected,
    details: Option<Value>,
}
/// Ordered native composition observations.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Expected {
    initial: Vec<String>,
    preview: Vec<String>,
    settled: Vec<String>,
    renders: usize,
    redraws_before: usize,
    redraws_after: usize,
    clears_before: usize,
    clears_after: usize,
    file_after: Option<String>,
}
/// Drive a requested differential frame to completion, without forcing redraw.
fn frame(ui: &TUI, runtime: &super::manual_runtime::ManualRuntime) {
    ui.request_render(false);
    runtime.settle().unwrap();
}
/// Count the full screen-clearing escape, not ordinary row clearing.
fn clears(terminal: &recording_terminal::RecordingTerminal) -> usize {
    terminal
        .writes()
        .iter()
        .filter(|s| s.contains("\x1b[2J\x1b[H\x1b[3J"))
        .count()
}
/// Source-backed native preview, replay or preflight-failure scenario.
pub fn run(name: &str) {
    let _guard = super::REGISTRY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _restore = super::Restore(maestro_tui::get_keybindings());
    super::keys(vec!["ctrl+o".into()]);
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../fixtures/edit_row_frames.json")).unwrap();
    assert_eq!(cases.len(), 3);
    let case = cases.into_iter().find(|c| c.test == name).unwrap();
    let (scratch, input) = setup(&case);
    let filename = case.input["path"].as_str().unwrap();
    let runtime = super::manual_runtime::ManualRuntime::new();
    let (params, requests) = super::params(&input, &runtime);
    let (ui, terminal) = writer(&case.kind, &params, &runtime);
    let row = Rc::new(ToolExecutionComponent::new(params).unwrap());
    ui.add_child(row.clone());
    frame(&ui, &runtime);
    assert_eq!(row.try_render(80).unwrap(), case.expected.initial);
    if case.kind == "replay" {
        row.update_result(
            ToolExecutionResult {
                content: vec![success_text()],
                details: case.details.clone(),
                is_error: false,
            },
            false,
        )
        .unwrap();
    } else {
        complete(&row, &runtime);
    }
    assert_eq!(row.try_render(80).unwrap(), case.expected.preview);
    frame(&ui, &runtime);
    assert_eq!(ui.full_redraws(), case.expected.redraws_before);
    assert_eq!(clears(&terminal), case.expected.clears_before);
    if case.kind == "large" {
        row.update_result(
            ToolExecutionResult {
                content: vec![success_text()],
                details: case.details,
                is_error: false,
            },
            false,
        )
        .unwrap();
    }
    assert_eq!(row.try_render(80).unwrap(), case.expected.settled);
    frame(&ui, &runtime);
    assert_eq!(ui.full_redraws(), case.expected.redraws_after);
    assert_eq!(clears(&terminal), case.expected.clears_after);
    assert_eq!(requests.get(), case.expected.renders);
    assert_eq!(
        std::fs::read_to_string(scratch.path(filename)).ok(),
        case.expected.file_after
    );
    assert_eq!(runtime.futures(), 0);
    assert_eq!(runtime.pending(), 0);
}

/// Visible success content that native diff renderers must suppress.
fn success_text() -> maestro_app::UserBlock {
    serde_json::from_value(json!({"type":"text","text":"native success text must stay hidden"}))
        .unwrap()
}

/// The existing recording terminal for containing-writer ownership tests.
pub fn terminal() -> recording_terminal::RecordingTerminal {
    recording_terminal::RecordingTerminal::new(80, 24)
}

/// Create source input independently from the expected post-operation file.
fn setup(case: &Case) -> (live::Scratch, super::records::Input) {
    let scratch = live::Scratch::new();
    let filename = case.input["path"].as_str().unwrap();
    let text = if case.kind == "preflight" {
        "line 0\nline 1\n".into()
    } else {
        (0..if case.kind == "replay" { 200 } else { 1000 }).fold(String::new(), |mut text, i| {
            use std::fmt::Write;
            writeln!(text, "line {i}").unwrap();
            text
        })
    };
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(scratch.path(filename))
        .unwrap()
        .write_all(text.as_bytes())
        .unwrap();
    if case.kind == "replay" {
        std::fs::remove_file(scratch.path(filename)).unwrap();
    }
    let input: super::records::Input =
        serde_json::from_value(json!({"tool":"edit","args":case.input,"cwd":scratch.path("")}))
            .unwrap();
    (scratch, input)
}
/// Await every admitted native preview future before observing its publication.
fn complete(row: &ToolExecutionComponent, runtime: &super::manual_runtime::ManualRuntime) {
    row.set_args_complete().unwrap();
    let work = runtime.take_futures();
    assert_eq!(work.len(), 1);
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            for future in work {
                future.await.unwrap();
            }
        });
}

/// Existing writer configured to preserve terminal history during shrinking updates.
fn writer(
    kind: &str,
    params: &maestro_chat::ToolExecutionParams,
    runtime: &super::manual_runtime::ManualRuntime,
) -> (TUI, recording_terminal::RecordingTerminal) {
    let terminal = recording_terminal::RecordingTerminal::new(80, 24);
    let ui = TUI::new(
        terminal.handle(),
        runtime.handle(),
        params.environment.terminal_image.clone(),
        Some(false),
    );
    ui.set_clear_on_shrink(false);
    if kind == "large" {
        for i in 0..200 {
            ui.add_child(Rc::new(Text::new(format!("history {i}"), 0, 0, None)));
        }
    }
    (ui, terminal)
}
