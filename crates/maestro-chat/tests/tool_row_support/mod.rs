//! Strict recorded queries and controlled renderer callbacks.
mod callbacks;
pub(crate) use crate::manual_runtime;
use crate::published;
mod records;
use crate::registry::REGISTRY;
use maestro_app::{ToolDef, ToolRenderEnvironment};
use maestro_chat::{ToolExecutionComponent, ToolExecutionOptions, ToolExecutionParams};
use maestro_theme::{ColorMode, ColorValue, Theme, ThemeBg, ThemeColor, ThemeOptions};
use maestro_tui::{
    Component, ImageProtocol, KeybindingDefinition, KeybindingKeys, KeybindingsManager,
    TerminalCapabilities, TerminalImage, get_keybindings, set_keybindings,
};
use records::{Corpus, Input, Operation};
use std::{cell::Cell, collections::HashSet, rc::Rc};

/// Restore the previous key registry on every exit.
struct Restore(KeybindingsManager);
impl Drop for Restore {
    fn drop(&mut self) {
        set_keybindings(self.0.clone());
    }
}
/// Register the supplied expand keys.
fn keys(value: Vec<String>) {
    set_keybindings(KeybindingsManager::new(
        vec![(
            "app.tools.expand".into(),
            KeybindingDefinition {
                default_keys: KeybindingKeys::Multiple(value),
                description: None,
            },
        )],
        vec![],
    ));
}
/// A palette built from the recording's actual color inputs.
fn theme(corpus: &Corpus, changed: bool) -> Rc<Theme> {
    let colors = corpus.colors.iter().map(|(k, v)| {
        (
            ThemeColor::Named(k.clone()),
            ColorValue::Index(if changed && k == "toolTitle" {
                61
            } else if changed && k == "toolOutput" {
                62
            } else {
                *v
            }),
        )
    });
    let bg = corpus.backgrounds.iter().map(|(k, v)| {
        (
            ThemeBg::Named(k.clone()),
            ColorValue::Index(if changed { *v + 50 } else { *v }),
        )
    });
    Rc::new(Theme::new(colors, bg, ColorMode::Color256, ThemeOptions::default()).unwrap())
}
/// Existing SDK factories selected by the application caller, never by the row.
fn builtin(input: &Input) -> Option<maestro_app::ToolRendererBinding> {
    let cwd = input.cwd.clone().unwrap_or_else(|| "/work".into());
    let definition = match input.tool.as_deref().unwrap_or("custom_tool") {
        "read" => ToolDef::Read(maestro_app::create_read_tool_definition::<()>(
            cwd,
            maestro_app::ReadToolOptions::default(),
        )),
        "edit" => ToolDef::Edit(maestro_app::create_edit_tool_definition::<()>(
            cwd,
            maestro_app::EditToolOptions::default(),
        )),
        "write" => ToolDef::Write(maestro_app::create_write_tool_definition::<()>(
            cwd,
            maestro_app::WriteToolOptions::default(),
        )),
        _ => return None,
    };
    Some(definition.renderer_binding().unwrap())
}
/// Shared terminal capabilities and explicitly observed environment.
fn environment(
    input: &Input,
    live: maestro_theme::LiveTheme,
    runtime: &manual_runtime::ManualRuntime,
) -> Rc<ToolRenderEnvironment> {
    let next = Cell::new(1);
    let terminal = TerminalImage::new(
        |_| None,
        move || {
            next.set(next.get() + 1);
            next.get()
        },
    );
    terminal.set_capabilities(TerminalCapabilities {
        images: input.protocol.as_deref().map(|p| match p {
            "kitty" => ImageProtocol::Kitty,
            "iterm2" => ImageProtocol::Iterm2,
            _ => panic!("unknown protocol"),
        }),
        true_color: true,
        hyperlinks: false,
    });
    Rc::new(ToolRenderEnvironment {
        runtime: runtime.handle(),
        readme_path: "/opt/maestro/README.md".into(),
        live_theme: live,
        highlighter: Rc::new(maestro_theme::SyntectHighlighter::new().unwrap()),
        terminal_image: terminal,
    })
}
/// Replay all observations, including callback order and request counts.
fn replay(corpus: &Corpus, case: &records::Case) {
    let input = &case.input;
    let (published, live) = published::published(&theme(corpus, false)).unwrap();
    keys(vec!["ctrl+o".into()]);
    let runtime = manual_runtime::ManualRuntime::new();
    let callbacks = Rc::new(callbacks::Callbacks::default());
    let (mut params, requests) = constructor(input, &runtime, live);
    params.tool_definition = callbacks.binding(input);
    let row = ToolExecutionComponent::new(params).unwrap();
    let mut frames = vec![row.try_render(input.width.unwrap_or(40)).unwrap()];
    let mut alias = None;
    for op in &input.ops {
        match &op.action {
            Operation::Args { value } => row.update_args(value.clone()).unwrap(),
            Operation::Started {} => row.mark_execution_started().unwrap(),
            Operation::Complete {} => row.set_args_complete().unwrap(),
            Operation::Result { value, partial } => row
                .update_result(value.convert(), partial.unwrap_or(false))
                .unwrap(),
            Operation::Expand { value } => row.set_expanded(*value).unwrap(),
            Operation::Show { value } => row.set_show_images(*value).unwrap(),
            Operation::ImageWidth { value } => row.set_image_width_cells(*value).unwrap(),
            Operation::Invalidate {} => row.invalidate(),
            Operation::Theme {} => published.set_theme_instance(theme(corpus, true)).unwrap(),
            Operation::Append { text } => {
                row.add_child(Rc::new(maestro_tui::Text::new(text.clone(), 0, 0, None)));
            }
            Operation::Capture {} => alias = Some(row.children()),
            Operation::AliasAppend { text } => alias
                .as_ref()
                .unwrap()
                .borrow_mut()
                .push(Rc::new(maestro_tui::Text::new(text.clone(), 0, 0, None))),
            Operation::Clear {} => row.clear(),
            Operation::Callback { slot } => callbacks.invalidate(slot),
            Operation::Keys { value } => keys(value.clone()),
            Operation::Flush {} => {
                assert_eq!(runtime.futures(), 0);
                assert!(runtime.run_due().into_iter().all(|r| r.is_ok()));
            }
        }
        frames.push(row.try_render(input.width.unwrap_or(40)).unwrap());
    }
    assert_eq!(frames, case.expected.frames, "frames {}", case.name);
    assert_eq!(
        *callbacks.events.borrow(),
        case.expected.events,
        "events {}",
        case.name
    );
    assert_eq!(
        requests.get(),
        case.expected.renders,
        "requests {}",
        case.name
    );
}
/// Reject unknown fields, duplicate full inputs and unregistered groups before replay.
pub fn group(name: &str) {
    let _guard = REGISTRY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _restore = Restore(get_keybindings());
    let text = include_str!("../fixtures/tool_rows.json");
    let corpus: Corpus = serde_json::from_str(text).unwrap();
    let raw: serde_json::Value = serde_json::from_str(text).unwrap();
    let mut unique = HashSet::new();
    for row in raw["cases"].as_array().unwrap() {
        assert!(unique.insert(row["input"].to_string()));
    }
    assert_eq!(corpus.cases.len(), 86);
    let groups: HashSet<_> = corpus.cases.iter().map(|c| c.test.as_str()).collect();
    assert_eq!(groups, super::FIXTURE_GROUPS.iter().copied().collect());
    let selected: Vec<_> = corpus.cases.iter().filter(|c| c.test == name).collect();
    assert!(!selected.is_empty(), "missing group {name}");
    for case in selected {
        replay(&corpus, case);
    }
}

pub mod native;
pub mod ownership;

/// Default input record for direct public-interface witnesses.
fn default_input() -> Input {
    serde_json::from_value(serde_json::json!({"tool":"custom_tool","definition":{},"args":{}}))
        .unwrap()
}
/// Reuses the recorded palette and environment for non-corpus witnesses.
fn params(
    input: &Input,
    runtime: &manual_runtime::ManualRuntime,
) -> (ToolExecutionParams, Rc<Cell<usize>>) {
    let corpus: Corpus = serde_json::from_str(include_str!("../fixtures/tool_rows.json")).unwrap();
    let (_, live) = published::published(&theme(&corpus, false)).unwrap();
    constructor(input, runtime, live)
}
/// Constructor routes consume options directly, without setter assistance.
fn constructor(
    input: &Input,
    runtime: &manual_runtime::ManualRuntime,
    live: maestro_theme::LiveTheme,
) -> (ToolExecutionParams, Rc<Cell<usize>>) {
    let requests = Rc::new(Cell::new(0));
    let counter = requests.clone();
    (
        ToolExecutionParams {
            tool_name: input.tool.clone().unwrap_or_else(|| "custom_tool".into()),
            tool_call_id: input.id.clone().unwrap_or_else(|| "tool-1".into()),
            args: input
                .args_json
                .as_ref()
                .map(|text| serde_json::from_str(text).unwrap())
                .or_else(|| input.args.clone()),
            options: ToolExecutionOptions {
                show_images: input
                    .options
                    .as_ref()
                    .and_then(|o| o.show_images)
                    .unwrap_or(true),
                image_width_cells: input
                    .options
                    .as_ref()
                    .and_then(|o| o.image_width_cells)
                    .unwrap_or(60.0),
            },
            tool_definition: None,
            built_in_tool_definition: builtin(input),
            environment: environment(input, live, runtime),
            cwd: input.cwd.clone().unwrap_or_else(|| "/work".into()),
            request_render: Rc::new(move || counter.set(counter.get() + 1)),
        },
        requests,
    )
}
