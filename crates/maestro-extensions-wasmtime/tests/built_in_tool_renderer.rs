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
        is_error: Some(false),
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
}

/// One literal text result, with no details or hidden inferred state.
fn text(value: &str) -> Value {
    json!({"content":[{"type":"text","text":value}]})
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
        let bash = binding(&runner, "bash");
        assert_eq!(
            paint(&bash, &text("one\n \ntwo\nexit code: 7"), [false; 2]),
            [format!(
                "{}{}",
                fg(&ThemeColor::Error, "exit 7"),
                fg(&ThemeColor::Dim, " (3 lines)")
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
