//! Compact built-in displays through the shipped component and production host.
#![cfg(test)]
use maestro_extensions::{
    ExtensionRunner, Local, ToolRenderContext, ToolRendererBinding, ToolRendererState,
};
use maestro_theme::{LiveTheme, ThemeColor};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    path::Path,
    rc::Rc,
    sync::{OnceLock, RwLock},
};

/// Loads the one shipped source with the normal component runtime.
async fn loaded() -> Rc<ExtensionRunner<Local>> {
    static COMPONENT: OnceLock<std::path::PathBuf> = OnceLock::new();
    let path = COMPONENT.get_or_init(|| {
        super::extension_guide::cargo::component(
            Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../maestro-extensions-wasm"
            )),
            &["--example", "built_in_tool_renderer"],
            "built_in_tool_renderer",
        )
        .unwrap()
    });
    let runtime = maestro_extensions::create_extension_runtime::<Local>();
    runtime.set_tool_construction(maestro_extensions::ToolConstruction {
        tools_manager: std::sync::Arc::new(maestro_extensions::ToolsManager::new(
            "/unused".into(),
            None,
        )),
        bin_dir: "/unused".into(),
    });
    let bus = super::components::TracedBus::new();
    let owner = maestro_extensions::load_extension_from_factory(
        crate::artifact(path.display().to_string()),
        std::env::current_dir().unwrap().to_str().unwrap().into(),
        bus.controller.for_extension(),
        runtime.clone(),
        None,
    )
    .await
    .unwrap();
    Rc::new(ExtensionRunner::<Local>::new(
        Rc::new(RwLock::new(vec![owner])),
        runtime,
        super::session::runner_session("/").0,
        Rc::new(|_| {}),
        LiveTheme::default(),
    ))
}

/// Supplies distinct semantic colors through the ordinary theme owner.
fn theme() -> LiveTheme {
    let colors = [
        ThemeColor::Warning,
        ThemeColor::Success,
        ThemeColor::Error,
        ThemeColor::Dim,
        ThemeColor::Muted,
        ThemeColor::ToolTitle,
        ThemeColor::Accent,
        ThemeColor::ToolOutput,
    ]
    .into_iter()
    .enumerate()
    .map(|(index, role)| {
        (
            role,
            maestro_theme::ColorValue::Index(u8::try_from(index + 1).unwrap()),
        )
    });
    LiveTheme::from_theme(Rc::new(
        maestro_theme::Theme::new(
            colors,
            [],
            maestro_theme::ColorMode::Color256,
            maestro_theme::ThemeOptions::default(),
        )
        .unwrap(),
    ))
}

/// Resolves one literal expected semantic role.
fn fg(role: &ThemeColor, text: &str) -> String {
    theme().get().unwrap().fg(role, text).unwrap()
}

/// No syntax engine is needed by these text-only custom renderers.
struct Plain;
impl maestro_theme::SyntaxHighlighter for Plain {
    fn supports_language(&self, _: &str) -> bool {
        false
    }
    fn highlight(
        &self,
        _: &str,
        _: &str,
    ) -> Result<Vec<maestro_theme::SyntaxSpan>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Vec::new())
    }
}

/// Supplies an actual retained native row without terminal or clock activity.
fn context<'a>(
    args: &'a Value,
    rows: &'a ToolRendererState,
) -> ToolRenderContext<'a, ToolRendererState> {
    ToolRenderContext {
        args: Some(args),
        tool_call_id: "compact".into(),
        invalidate: Rc::new(|| {}),
        environment: Rc::new(maestro_extensions::ToolRenderEnvironment {
            runtime: super::manual_runtime::ManualRuntime::new().handle(),
            readme_path: String::new(),
            live_theme: LiveTheme::default(),
            highlighter: Rc::new(Plain),
            terminal_image: maestro_tui::TerminalImage::default(),
        }),
        last_component: None,
        state: Rc::new(RefCell::new(rows.clone())),
        renderer_state: Some(rows),
        cwd: "/".into(),
        phase: maestro_extensions::ToolRenderPhase {
            execution_started: true,
            args_complete: true,
            is_partial: false,
        },
        expanded: false,
        show_images: false,
        is_error: false,
    }
}

/// Gets the renderer installed under the original built-in name.
fn binding(runner: &ExtensionRunner<Local>, name: &str) -> ToolRendererBinding {
    runner
        .get_all_registered_tools()
        .iter()
        .find(|tool| tool.definition.name() == name)
        .unwrap()
        .definition
        .renderer_binding()
        .unwrap()
}

/// Renders independently supplied content, details and expansion/progress options.
fn paint(binding: &ToolRendererBinding, value: &Value, flags: [bool; 2]) -> Vec<String> {
    let content: Vec<_> = serde_json::from_value(value["content"].clone()).unwrap();
    let result = maestro_extensions::ToolRenderResultLike {
        content: &content,
        details: value.get("details"),
        terminate: None,
        is_error: Some(value["isError"] == true),
    };
    let rows = ToolRendererState::default();
    let args = json!({});
    binding
        .render_result
        .as_ref()
        .unwrap()
        .prepare(result.with_details(result.details))
        .unwrap()
        .render(
            maestro_extensions::ToolRenderResultOptions {
                expanded: flags[0],
                is_partial: flags[1],
            },
            &theme(),
            &context(&args, &rows),
        )
        .unwrap()
        .try_render(200)
        .unwrap()
        .into_iter()
        .map(|line| line.trim_end().to_owned())
        .collect()
}

/// One literal text result, with no details or hidden inferred state.
fn text(value: &str) -> Value {
    json!({"content":[{"type":"text","text":value}]})
}

/// Checks shell status without mistaking successful output for an exit failure.
fn shell_results(runner: &ExtensionRunner<Local>) {
    let bash = binding(runner, "bash");
    let mut shell_error = text("one\n \ntwo\nexit code: 7");
    shell_error["isError"] = json!(true);
    assert_eq!(
        paint(&bash, &shell_error, [false; 2]),
        [format!(
            "{}{}",
            fg(&ThemeColor::Error, "exit 7"),
            fg(&ThemeColor::Dim, " (3 lines)")
        )]
    );
    assert_eq!(
        paint(&bash, &text("exit code: 7"), [false; 2]),
        [format!(
            "{}{}",
            fg(&ThemeColor::Success, "done"),
            fg(&ThemeColor::Dim, " (1 lines)")
        )]
    );
    assert_eq!(
        paint(&bash, &json!({"content":[]}), [false; 2]),
        [format!(
            "{}{}",
            fg(&ThemeColor::Success, "done"),
            fg(&ThemeColor::Dim, " (0 lines)")
        )]
    );
}

#[test]
fn built_in_tool_renderer_expands_timeout_and_cancellation_diagnostics() {
    super::wasi_support::run(async {
        let runner = loaded().await;
        let bash = binding(&runner, "bash");
        for reason in ["Command timed out after 5 seconds", "Command aborted"] {
            let mut value = text(&format!("progress\n\n{reason}"));
            value["isError"] = json!(true);
            value["details"] = json!({"truncation":{"truncated":true}});
            let heading = format!(
                "{}{}{}",
                fg(&ThemeColor::Error, "progress"),
                fg(&ThemeColor::Dim, " (2 lines)"),
                fg(&ThemeColor::Warning, " [truncated]")
            );
            assert_eq!(
                paint(&bash, &value, [true, false]),
                [
                    heading.clone(),
                    fg(&ThemeColor::Dim, "progress"),
                    String::new(),
                    fg(&ThemeColor::Dim, reason),
                ]
            );
            assert_eq!(paint(&bash, &value, [false; 2]), [heading]);
        }
    });
}

#[test]
fn built_in_tool_renderer_displays_progress_content_and_failures() {
    super::wasi_support::run(async {
        let runner = loaded().await;
        for (name, progress) in [
            ("read", "Reading..."),
            ("bash", "Running..."),
            ("edit", "Editing..."),
            ("write", "Writing..."),
        ] {
            let binding = binding(&runner, name);
            assert_eq!(
                paint(&binding, &json!({"content":[]}), [false, true]),
                [fg(&ThemeColor::Warning, progress)]
            );
        }
        let read = binding(&runner, "read");
        assert_eq!(
            paint(&read, &text("one\ntwo\n"), [false; 2]),
            [fg(&ThemeColor::Success, "3 lines")]
        );
        assert_eq!(
            paint(&read, &json!({"content":[]}), [false; 2]),
            [fg(&ThemeColor::Error, "No content")]
        );
        assert_eq!(
            paint(
                &read,
                &json!({"content":[{"type":"image","data":"AA==","mimeType":"image/png"}]}),
                [false; 2]
            ),
            [fg(&ThemeColor::Success, "Image loaded")]
        );
        shell_results(&runner);
        for (name, success) in [("edit", "Applied"), ("write", "Written")] {
            let binding = binding(&runner, name);
            assert_eq!(
                paint(&binding, &text("ok"), [false; 2]),
                [fg(&ThemeColor::Success, success)]
            );
            assert_eq!(
                paint(&binding, &text("Error: refused\nextra"), [false; 2]),
                [fg(&ThemeColor::Error, "Error: refused")]
            );
        }
    });
}

/// Paints the registered call renderer in a real native row.
fn call(binding: &ToolRendererBinding, args: &Value) -> Vec<String> {
    let rows = ToolRendererState::default();
    binding.render_call.as_ref().unwrap()(Some(args), &theme(), &context(args, &rows))
        .unwrap()
        .try_render(200)
        .unwrap()
        .into_iter()
        .map(|line| line.trim_end().to_owned())
        .collect()
}

/// Checks authored call labels, numeric ranges, content counts and command clipping.
fn compact_calls(runner: &ExtensionRunner<Local>) {
    for (name, args, operand, suffix) in [
        (
            "read",
            json!({"path":"src.rs","offset":2,"limit":3}),
            "src.rs",
            " (offset=2, limit=3)",
        ),
        (
            "read",
            json!({"path":"src.rs","offset":0,"limit":0}),
            "src.rs",
            "",
        ),
        ("edit", json!({"path":"src.rs"}), "src.rs", ""),
        (
            "write",
            json!({"path":"src.rs","content":"a\nb\n"}),
            "src.rs",
            " (3 lines)",
        ),
    ] {
        let title = fg(
            &ThemeColor::ToolTitle,
            &maestro_theme::Theme::bold(&format!("{name} "), true),
        );
        assert_eq!(
            call(&binding(runner, name), &args),
            [format!(
                "{title}{}{}",
                fg(&ThemeColor::Accent, operand),
                if suffix.is_empty() {
                    String::new()
                } else {
                    fg(&ThemeColor::Dim, suffix)
                }
            )]
        );
    }
    let title = fg(
        &ThemeColor::ToolTitle,
        &maestro_theme::Theme::bold("$ ", true),
    );
    for (command, expected) in [
        ("x".repeat(80), "x".repeat(80)),
        ("x".repeat(81), format!("{}...", "x".repeat(77))),
        ("😀".repeat(41), format!("{}...", "😀".repeat(38))),
    ] {
        assert_eq!(
            call(
                &binding(runner, "bash"),
                &json!({"command":command,"timeout":5})
            ),
            [format!(
                "{title}{}{}",
                fg(&ThemeColor::Accent, &expected),
                fg(&ThemeColor::Dim, " (timeout: 5s)")
            )]
        );
    }
}

/// Checks the read and shell preview limits and truncation warnings independently.
fn content_previews(runner: &ExtensionRunner<Local>) {
    let read = json!({"content":[{"type":"text","text":format!("{}tail", "x\n".repeat(15))}],
        "details":{"truncation":{"truncated":true,"totalLines":99}}});
    let heading = format!(
        "{}{}",
        fg(&ThemeColor::Success, "16 lines"),
        fg(&ThemeColor::Warning, " (truncated from 99)")
    );
    assert_eq!(
        paint(&binding(runner, "read"), &read, [false; 2]),
        std::slice::from_ref(&heading)
    );
    let mut expected = vec![heading];
    expected.extend(vec![fg(&ThemeColor::Dim, "x"); 15]);
    expected.push(fg(&ThemeColor::Muted, "... 1 more lines"));
    assert_eq!(
        paint(&binding(runner, "read"), &read, [true, false]),
        expected
    );
    let bash = json!({"content":[{"type":"text","text":format!("{}last", "x\n".repeat(20))}],
        "details":{"truncation":{"truncated":true}}});
    let heading = format!(
        "{}{}{}",
        fg(&ThemeColor::Success, "done"),
        fg(&ThemeColor::Dim, " (21 lines)"),
        fg(&ThemeColor::Warning, " [truncated]")
    );
    assert_eq!(
        paint(&binding(runner, "bash"), &bash, [false; 2]),
        std::slice::from_ref(&heading)
    );
    let mut expected = vec![heading];
    expected.extend(vec![fg(&ThemeColor::Dim, "x"); 20]);
    expected.push(fg(&ThemeColor::Muted, "... more output"));
    assert_eq!(
        paint(&binding(runner, "bash"), &bash, [true, false]),
        expected
    );
    assert_eq!(
        paint(&binding(runner, "bash"), &text("\u{feff}\n"), [false; 2]),
        [format!(
            "{}{}",
            fg(&ThemeColor::Success, "done"),
            fg(&ThemeColor::Dim, " (0 lines)")
        )]
    );
}

/// Checks diff headers, additions, removals and the thirty-line expanded limit.
fn diff_preview(runner: &ExtensionRunner<Local>) {
    let result = json!({"content":[],"details":{"diff":format!("+++ file\n--- file\n+add\n-remove\n same\n{}", "+x\n".repeat(26))}});
    let heading = format!(
        "{}{}{}",
        fg(&ThemeColor::Success, "+27"),
        fg(&ThemeColor::Dim, " / "),
        fg(&ThemeColor::Error, "-1")
    );
    let edit = binding(runner, "edit");
    assert!(matches!(
        edit.render_shell,
        Some(maestro_extensions::ToolRenderShell::SelfRendered)
    ));
    assert_eq!(
        paint(&edit, &result, [false; 2]),
        std::slice::from_ref(&heading)
    );
    let mut expected = vec![
        heading,
        fg(&ThemeColor::Dim, "+++ file"),
        fg(&ThemeColor::Dim, "--- file"),
        fg(&ThemeColor::Success, "+add"),
        fg(&ThemeColor::Error, "-remove"),
        fg(&ThemeColor::Dim, " same"),
    ];
    expected.extend(vec![fg(&ThemeColor::Success, "+x"); 25]);
    expected.push(fg(&ThemeColor::Muted, "... 2 more diff lines"));
    assert_eq!(paint(&edit, &result, [true, false]), expected);
}

#[test]
fn built_in_tool_renderer_compacts_calls_and_expanded_results() {
    super::wasi_support::run(async {
        let runner = loaded().await;
        compact_calls(&runner);
        content_previews(&runner);
        diff_preview(&runner);
    });
}

/// Executes a registered tool through the same wrapper used by an application session.
async fn execute(
    runner: &Rc<ExtensionRunner<Local>>,
    name: &str,
    args: Value,
) -> Result<Value, String> {
    let registered = runner.get_all_registered_tools();
    let registered = registered
        .iter()
        .find(|tool| tool.definition.name() == name)
        .unwrap();
    assert_eq!(registered.metadata().label, name);
    assert!(!registered.metadata().definition.description.is_empty());
    let tool = maestro_extensions::wrap_registered_tool(registered, runner.clone());
    (tool.execute)("delegated".into(), args, None, None)
        .await
        .map(|result| serde_json::to_value(result).unwrap())
        .map_err(|error| error.to_string())
}

/// Uses disposable files and a shell command with fixed text and no repository effects.
async fn delegated_execution(runner: &Rc<ExtensionRunner<Local>>) -> String {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let path = scratch.path().join("note.txt");
    let names: Vec<_> = runner
        .get_all_registered_tools()
        .iter()
        .map(|tool| tool.definition.name())
        .collect();
    assert_eq!(names, ["read", "bash", "edit", "write"]);
    let written = execute(runner, "write", json!({"path":path,"content":"alpha\n"}))
        .await
        .unwrap();
    assert_eq!(
        written["content"][0]["text"],
        format!("Successfully wrote 6 bytes to {}", path.display())
    );
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "alpha\n");
    let read = execute(runner, "read", json!({"path":path})).await.unwrap();
    assert_eq!(read["content"][0]["text"], "alpha\n");
    let edited = execute(
        runner,
        "edit",
        json!({"path":path,"edits":[{"oldText":"alpha","newText":"beta"}]}),
    )
    .await
    .unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "beta\n");
    assert!(edited["details"]["diff"].as_str().unwrap().contains("beta"));
    let shell = execute(runner, "bash", json!({"command":"printf 'controlled\\n'"}))
        .await
        .unwrap();
    assert_eq!(shell["content"][0]["text"], "controlled\n");
    execute(
        runner,
        "bash",
        json!({"command":"printf controlled; exit 7"}),
    )
    .await
    .unwrap_err()
}

#[test]
fn built_in_tool_renderer_delegates_execution_and_displays_native_failures() {
    super::wasi_support::run(async {
        let runner = loaded().await;
        let error = delegated_execution(&runner).await;
        assert_eq!(error, "controlled\n\nCommand exited with code 7");
        let mut value = text(&error);
        value["isError"] = json!(true);
        assert_eq!(
            paint(&binding(&runner, "bash"), &value, [false; 2]),
            [format!(
                "{}{}",
                fg(&ThemeColor::Error, "exit 7"),
                fg(&ThemeColor::Dim, " (2 lines)")
            )]
        );
        for name in ["read", "edit", "write", "bash"] {
            let mut value = text("ENOENT: controlled\nextra");
            value["isError"] = json!(true);
            assert_eq!(
                paint(&binding(&runner, name), &value, [false; 2]),
                [if name == "bash" {
                    format!(
                        "{}{}",
                        fg(&ThemeColor::Error, "ENOENT: controlled"),
                        fg(&ThemeColor::Dim, " (2 lines)")
                    )
                } else {
                    fg(&ThemeColor::Error, "ENOENT: controlled")
                }]
            );
        }
    });
}
