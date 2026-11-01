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
    let (published, live) = published::published(&theme(corpus, false));
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
    let (_, live) = published::published(&theme(&corpus, false));
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
/// Generic rows print a retained call's complete argument value as two-space JSON.
pub fn argument_roots() {
    let _guard = REGISTRY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _restore = Restore(get_keybindings());
    keys(vec!["ctrl+o".into()]);
    for (arguments, printed) in [
        (serde_json::json!(null), vec!["null"]),
        (serde_json::json!(false), vec!["false"]),
        (serde_json::json!(0), vec!["0"]),
        (serde_json::json!(""), vec![r#""""#]),
        (serde_json::json!([]), vec!["[]"]),
        (
            serde_json::json!(["a", [1, {"k": null}]]),
            vec![
                "[",
                r#"  "a","#,
                "  [",
                "    1,",
                "    {",
                r#"      "k": null"#,
                "    }",
                "  ]",
                "]",
            ],
        ),
        (serde_json::json!({}), vec!["{}"]),
        (
            serde_json::json!({"z": 1, "a": [false]}),
            vec!["{", r#"  "z": 1,"#, r#"  "a": ["#, "    false", "  ]", "}"],
        ),
    ] {
        argument_root(&arguments, &printed);
    }
}
/// Render one retained call's row before and after a result, comparing every styled line.
fn argument_root(arguments: &serde_json::Value, printed: &[&str]) {
    let call: maestro_app::ToolCall = serde_json::from_value(serde_json::json!({
        "type": "toolCall", "id": "tool-1", "name": "custom_tool", "arguments": arguments,
    }))
    .unwrap();
    let runtime = manual_runtime::ManualRuntime::new();
    let (mut params, _) = params(&default_input(), &runtime);
    params.args = Some(call.arguments.clone());
    let component = ToolExecutionComponent::new(params).unwrap();
    let row = |bg: u8, text: &str| format!("\x1b[48;5;{bg}m {text:<39}\x1b[49m");
    let frame = |bg: u8, output: Option<&str>| {
        let mut lines = vec![String::new(), row(bg, "")];
        lines.push(format!(
            "\x1b[48;5;{bg}m \x1b[38;5;1m\x1b[1mcustom_tool\x1b[22m\x1b[39m{:<28}\x1b[49m",
            ""
        ));
        lines.push(row(bg, ""));
        lines.extend(printed.iter().map(|text| row(bg, text)));
        lines.extend(output.map(|text| row(bg, text)));
        lines.push(row(bg, ""));
        lines
    };
    let label = call.arguments.to_string();
    assert_eq!(
        component.try_render(40).unwrap(),
        frame(13, None),
        "{label}"
    );
    let done = maestro_app::UserBlock::Text(maestro_app::TextContent::new("done".into()));
    component
        .update_result(
            maestro_chat::ToolExecutionResult {
                content: vec![done],
                details: None,
                is_error: false,
            },
            false,
        )
        .unwrap();
    assert_eq!(
        component.try_render(40).unwrap(),
        frame(14, Some("done")),
        "{label}"
    );
}
/// A self-rendered row whose callbacks draw nothing and that has no images takes no rows,
/// before and after its result.
pub fn empty_self_rendered() {
    use maestro_app::{ToolRenderResultCallback, ToolRenderShell, ToolRendererBinding};
    let runtime = manual_runtime::ManualRuntime::new();
    let (mut params, _) = params(&default_input(), &runtime);
    let result: ToolRenderResultCallback<serde_json::Value, maestro_app::ToolRendererState> =
        Rc::new(|_, _, _, _| Ok(Rc::new(maestro_tui::Text::new(String::new(), 0, 0, None))));
    params.tool_definition = Some(ToolRendererBinding {
        render_call: Some(Rc::new(|_, _, _| {
            Ok(Rc::new(maestro_tui::Text::new(String::new(), 0, 0, None)))
        })),
        render_result: Some(result.into()),
        render_shell: Some(ToolRenderShell::SelfRendered),
    });
    let row = ToolExecutionComponent::new(params).unwrap();
    assert_eq!(row.render(120), Vec::<String>::new());
    row.update_result(
        maestro_chat::ToolExecutionResult {
            content: vec![],
            details: Some(serde_json::json!({})),
            is_error: false,
        },
        false,
    )
    .unwrap();
    assert_eq!(row.render(120), Vec::<String>::new());
}
/// A result without a result renderer shows its first ten lines and a hint until expanded.
pub fn collapsed_fallback() {
    use maestro_app::{ToolRenderShell, ToolRendererBinding};
    let _guard = REGISTRY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _restore = Restore(get_keybindings());
    keys(vec!["ctrl+o".into()]);
    let runtime = manual_runtime::ManualRuntime::new();
    let (mut params, _) = params(&default_input(), &runtime);
    params.tool_definition = Some(ToolRendererBinding {
        render_call: Some(Rc::new(|_, _, _| {
            Ok(Rc::new(maestro_tui::Text::new("call".into(), 0, 0, None)))
        })),
        render_result: None,
        render_shell: Some(ToolRenderShell::Default),
    });
    let row = ToolExecutionComponent::new(params).unwrap();
    let plain = |row: &ToolExecutionComponent| -> Vec<String> {
        row.render(120)
            .iter()
            .map(|line| maestro_app::strip_ansi(line).unwrap().trim_end().to_owned())
            .collect()
    };
    for (lines, hidden) in [(15, Some(5)), (100, Some(90)), (10, None)] {
        let text = (1..=lines)
            .map(|line| format!("line-{line}"))
            .collect::<Vec<_>>()
            .join("\n");
        row.set_expanded(false).unwrap();
        row.update_result(
            maestro_chat::ToolExecutionResult {
                content: vec![maestro_app::UserBlock::Text(maestro_app::TextContent::new(
                    text,
                ))],
                details: None,
                is_error: false,
            },
            false,
        )
        .unwrap();
        let collapsed = plain(&row);
        assert!(collapsed.iter().any(|line| line.trim() == "line-10"));
        assert!(!collapsed.iter().any(|line| line.trim() == "line-11"));
        let hint = hidden.map(|hidden| format!("... ({hidden} more lines, ctrl+o to expand)"));
        assert_eq!(
            collapsed
                .iter()
                .find(|line| line.contains("more lines"))
                .map(|line| line.trim().to_owned()),
            hint,
            "{collapsed:?}"
        );
        row.set_expanded(true).unwrap();
        let expanded = plain(&row);
        assert!(
            expanded
                .iter()
                .any(|line| line.trim() == format!("line-{lines}"))
        );
        assert!(!expanded.iter().any(|line| line.contains("more lines")));
    }
}
