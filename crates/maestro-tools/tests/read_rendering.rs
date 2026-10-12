//! How the read definition draws its calls and results.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

mod display_support;

use display_support::{
    Env, Setup, WIDTH, assert_rows, bare_theme, bind, cases, check_rows, context, padded,
    theme_keys,
};
use futures_util::FutureExt;
use maestro_agent::AgentToolResult;
use maestro_models::{BoxFuture, DiagnosticErrorInfo, TextContent, UserBlock};
use maestro_theme::{SyntaxHighlighter, SyntaxSpan, ThemeColor};
use maestro_tools::{
    ReadOperations, ReadToolDetails, ReadToolInput, ReadToolOptions, ToolRenderResultOptions,
    create_read_tool_definition, output::render_utils::strip_ansi, wrap_tool_definition,
};
use maestro_tui::{Component, Text, tui::ComponentHandle};
use serde_json::{Value, json};
use std::{cell::RefCell, error::Error, rc::Rc, sync::Arc};

/// Records every highlight request and classifies the whole text as a keyword.
struct Probe {
    /// Requests as (code, language).
    calls: RefCell<Vec<(String, String)>>,
    /// Whether the engine claims every language.
    supported: bool,
}

impl Probe {
    fn new(supported: bool) -> Rc<Self> {
        Rc::new(Self {
            calls: RefCell::default(),
            supported,
        })
    }
}

impl SyntaxHighlighter for Probe {
    fn supports_language(&self, _: &str) -> bool {
        self.supported
    }

    fn highlight(
        &self,
        code: &str,
        language: &str,
    ) -> Result<Vec<SyntaxSpan>, Box<dyn Error + Send + Sync>> {
        self.calls
            .borrow_mut()
            .push((code.to_owned(), language.to_owned()));
        Ok(vec![SyntaxSpan {
            range: 0..code.len(),
            color: Some(ThemeColor::SyntaxKeyword),
        }])
    }
}

/// The palette color of a theme key, before any shift.
fn color(key: &str) -> usize {
    theme_keys().iter().position(|name| name == key).unwrap() + 1
}

/// A setup with the expand key bound.
fn setup() -> Setup {
    Setup::from(
        &json!({"readme": "/opt/maestro/README.md", "bindings": ["ctrl+o"], "expanded": true}),
    )
}

/// The tool whose renderers are under test.
fn tool() -> maestro_tools::ToolDefinition<(), ReadToolInput, ReadToolDetails> {
    create_read_tool_definition::<()>("/work".into(), ReadToolOptions::default())
}

/// A result of one text block.
fn text_result(text: &str) -> AgentToolResult<ReadToolDetails> {
    let text = UserBlock::Text(TextContent {
        text: text.into(),
        text_signature: None,
    });
    AgentToolResult {
        content: vec![text],
        details: None,
        terminate: None,
    }
}

/// The options of a settled result.
fn settled(expanded: bool) -> ToolRenderResultOptions {
    ToolRenderResultOptions {
        expanded,
        is_partial: false,
    }
}

#[test]
fn read_titles_keep_argument_precedence_and_invalid_styles() {
    check_rows(
        "read_titles_keep_argument_precedence_and_invalid_styles",
        false,
    );
}

#[test]
fn read_line_ranges_preserve_numeric_spelling_and_presence() {
    check_rows(
        "read_line_ranges_preserve_numeric_spelling_and_presence",
        false,
    );
}

#[test]
fn read_classification_obeys_skill_docs_resource_priority() {
    check_rows(
        "read_classification_obeys_skill_docs_resource_priority",
        false,
    );
}

#[test]
fn classified_reads_hide_only_successful_compact_results() {
    check_rows(
        "classified_reads_hide_only_successful_compact_results",
        false,
    );
}

#[test]
fn expanded_read_titles_return_to_authored_paths() {
    check_rows("expanded_read_titles_return_to_authored_paths", false);
}

#[test]
fn expanded_read_results_reveal_classified_content() {
    check_rows("expanded_read_results_reveal_classified_content", false);
}

#[test]
fn read_errors_bypass_compact_hiding() {
    check_rows("read_errors_bypass_compact_hiding", false);
}

#[test]
fn compact_read_ranges_precede_live_expand_hints() {
    check_rows("compact_read_ranges_precede_live_expand_hints", false);
}

#[test]
fn read_context_uses_current_cwd_and_documentation_path() {
    check_rows(
        "read_context_uses_current_cwd_and_documentation_path",
        false,
    );
}

#[test]
fn read_preview_limits_count_logical_lines_before_wrapping() {
    check_rows(
        "read_preview_limits_count_logical_lines_before_wrapping",
        false,
    );
    let _bindings = bind(&["ctrl+o".to_owned()]);
    let env = Env::new(0);
    let args = json!({"path": "notes.txt"});
    let long = "x".repeat(50);
    let lines: Vec<String> = std::iter::once(long)
        .chain((2..=12).map(|n| format!("line {n}")))
        .collect();
    let setup = Setup::from(&json!({"readme": "/opt/maestro/README.md", "bindings": ["ctrl+o"]}));
    let ctx = context(&args, &setup, &env, None);
    let row = (tool().render_result.unwrap())(
        &text_result(&lines.join("\n")),
        settled(false),
        &env.theme,
        &ctx,
    )
    .unwrap();
    let rows: Vec<String> = row
        .render(40)
        .iter()
        .map(|row| strip_ansi(row).unwrap())
        .collect();
    assert_eq!(rows.len(), 13);
    assert_eq!(rows[1].trim_end(), "x".repeat(40));
    assert_eq!(rows[2].trim_end(), "x".repeat(10));
    assert_eq!(rows[12].trim_end(), "... (2 more lines, ctrl+o to expand)");
}

#[test]
fn read_trailing_empty_rows_do_not_trim_visible_whitespace() {
    check_rows(
        "read_trailing_empty_rows_do_not_trim_visible_whitespace",
        false,
    );
}

#[test]
fn read_truncation_notices_follow_reason_and_explicit_limits() {
    check_rows(
        "read_truncation_notices_follow_reason_and_explicit_limits",
        false,
    );
}

#[test]
fn read_highlighting_uses_only_selected_filename_language() {
    check_rows(
        "read_highlighting_uses_only_selected_filename_language",
        true,
    );
    let keyword = color("syntaxKeyword");
    let code_block = color("mdCodeBlock");
    for (args, supported, calls, row) in [
        (
            json!({"path": "code.rs"}),
            true,
            vec![("a   b".to_owned(), "rust".to_owned())],
            format!("\x1b[38;5;{keyword}ma   b\x1b[39m"),
        ),
        (
            json!({"path": "CODE.RS"}),
            true,
            vec![("a   b".to_owned(), "rust".to_owned())],
            format!("\x1b[38;5;{keyword}ma   b\x1b[39m"),
        ),
        (
            json!({"path": "other.txt", "file_path": "chosen.rs"}),
            true,
            vec![("a   b".to_owned(), "rust".to_owned())],
            format!("\x1b[38;5;{keyword}ma   b\x1b[39m"),
        ),
        (
            json!({"path": "code.unknown"}),
            true,
            vec![],
            format!("\x1b[38;5;{}ma   b\x1b[39m", color("toolOutput")),
        ),
        (
            json!({"path": "code.rs"}),
            false,
            vec![],
            format!("\x1b[38;5;{code_block}ma   b\x1b[39m"),
        ),
    ] {
        let probe = Probe::new(supported);
        let env = Env::with(
            Rc::new(display_support::palette(0)),
            Rc::clone(&probe) as Rc<dyn SyntaxHighlighter>,
        );
        let ctx = context(&args, &setup(), &env, None);
        let row_component =
            (tool().render_result.unwrap())(&text_result("a\tb"), settled(true), &env.theme, &ctx)
                .unwrap();
        assert_eq!(*probe.calls.borrow(), calls, "{args} supported={supported}");
        assert_eq!(
            row_component.render(WIDTH)[1],
            padded(&row),
            "{args} supported={supported}"
        );
    }
}

#[test]
fn read_highlighting_reads_the_current_published_theme() {
    check_rows("read_highlighting_reads_the_current_published_theme", true);
    let keyword = color("syntaxKeyword");
    let probe = Probe::new(true);
    let mut env = Env::with(
        Rc::new(display_support::palette(0)),
        Rc::clone(&probe) as Rc<dyn SyntaxHighlighter>,
    );
    let before_theme = Rc::clone(&env.theme);
    let args = json!({"path": "main.rs"});
    let render = |env: &Env| {
        let ctx = context(&args, &setup(), env, None);
        (tool().render_result.unwrap())(
            &text_result("fn main() {}"),
            settled(true),
            &before_theme,
            &ctx,
        )
        .unwrap()
        .render(WIDTH)[1]
            .clone()
    };
    assert_eq!(
        render(&env),
        padded(&format!("\x1b[38;5;{keyword}mfn main() {{}}\x1b[39m"))
    );
    env.republish(40);
    assert_eq!(
        render(&env),
        padded(&format!(
            "\x1b[38;5;{}mfn main() {{}}\x1b[39m",
            keyword + 40
        ))
    );
}

#[test]
fn read_image_fallback_obeys_capability_and_visibility() {
    check_rows("read_image_fallback_obeys_capability_and_visibility", false);
}

#[test]
fn read_image_indicators_keep_order_and_dimension_fallbacks() {
    check_rows(
        "read_image_indicators_keep_order_and_dimension_fallbacks",
        false,
    );
}

#[test]
fn read_output_sanitizes_before_tab_expansion_and_layout() {
    check_rows(
        "read_output_sanitizes_before_tab_expansion_and_layout",
        false,
    );
}

#[test]
fn read_result_uses_options_expansion_not_unrelated_flags() {
    check_rows(
        "read_result_uses_options_expansion_not_unrelated_flags",
        false,
    );
}

#[test]
fn read_renderers_reuse_the_supplied_text_slot() {
    let _bindings = bind(&["ctrl+o".to_owned()]);
    let env = Env::new(0);
    let mut survivors = Vec::new();
    for case in cases("read_renderers_reuse_the_supplied_text_slot") {
        let prior = Rc::new(Text::new("before".into(), 0, 0, None));
        let handle: ComponentHandle = Rc::clone(&prior) as ComponentHandle;
        let (args, ctx_args) = (json!({"path": "notes.txt"}), json!({"path": "after.txt"}));
        let ctx = context(
            if case.input["slot"] == "call" {
                &ctx_args
            } else {
                &args
            },
            &setup(),
            &env,
            Some(Rc::clone(&handle)),
        );
        let returned = if case.input["slot"] == "call" {
            (tool().render_call.unwrap())(&ctx_args, &env.theme, &ctx).unwrap()
        } else {
            (tool().render_result.unwrap())(&text_result("after"), settled(true), &env.theme, &ctx)
                .unwrap()
        };
        assert_eq!(
            case.expected["same"],
            Value::Bool(Rc::ptr_eq(&returned, &handle)),
            "case {}",
            case.id
        );
        let rows: Vec<String> = case.expected["lines"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| padded(row.as_str().unwrap()))
            .collect();
        assert_eq!(prior.render(WIDTH), rows, "case {}", case.id);
        survivors.push((case.id, Rc::downgrade(&prior)));
        drop((ctx, returned, handle, prior));
    }
    assert_eq!(survivors.len(), 2);
    assert!(survivors.iter().all(|(_, weak)| weak.upgrade().is_none()));
}

#[test]
fn read_renderer_errors_leave_previous_text_unchanged() {
    let env = Env::with(
        Rc::new(bare_theme()),
        Rc::new(Probe {
            calls: RefCell::default(),
            supported: true,
        }),
    );
    for case in cases("read_renderer_errors_leave_previous_text_unchanged") {
        let prior = Rc::new(Text::new("before".into(), 0, 0, None));
        let args = json!({"path": "after.txt"});
        let ctx = context(
            &args,
            &setup(),
            &env,
            Some(Rc::clone(&prior) as ComponentHandle),
        );
        let error = (tool().render_call.unwrap())(&args, &env.theme, &ctx)
            .err()
            .unwrap();
        assert_eq!(
            Value::from(error.message.as_str()),
            case.expected["error"],
            "case {}",
            case.id
        );
        let rows: Vec<String> = case.expected["lines"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| padded(row.as_str().unwrap()))
            .collect();
        assert_eq!(prior.render(WIDTH), rows, "case {}", case.id);
    }
}

#[test]
fn classified_results_avoid_unused_display_dependencies() {
    let probe = Probe::new(true);
    let env = Env::with(
        Rc::new(bare_theme()),
        Rc::clone(&probe) as Rc<dyn SyntaxHighlighter>,
    );
    let args = json!({"path": "/work/SKILL.md"});
    let mut ctx = context(
        &args,
        &Setup::from(&json!({"readme": "/opt/maestro/README.md", "bindings": []})),
        &env,
        None,
    );
    for case in cases("classified_results_avoid_unused_display_dependencies") {
        let hidden = (tool().render_result.as_ref().unwrap())(
            &text_result("secret"),
            settled(false),
            &env.theme,
            &ctx,
        )
        .unwrap();
        assert_rows(&case, &hidden.render(WIDTH), false);
        assert!(probe.calls.borrow().is_empty());
    }
    ctx.is_error = true;
    let overflow = (1..=12)
        .map(|n| format!("line {n}"))
        .collect::<Vec<_>>()
        .join("\n");
    let error =
        (tool().render_result.unwrap())(&text_result(&overflow), settled(false), &env.theme, &ctx)
            .err()
            .unwrap();
    assert_eq!(error.message, "Unknown theme color: muted");
}

/// File access that serves one fixed file.
struct Fixed;

impl ReadOperations for Fixed {
    fn read_file(&self, _: String) -> BoxFuture<Result<Vec<u8>, DiagnosticErrorInfo>> {
        Box::pin(std::future::ready(Ok(b"served".to_vec())))
    }

    fn access(&self, _: String) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        Box::pin(std::future::ready(Ok(())))
    }
}

#[test]
fn read_definition_keeps_render_alias_separate_from_execution_input() {
    let _bindings = bind(&["ctrl+o".to_owned()]);
    let env = Env::new(0);
    let options = ReadToolOptions {
        operations: Some(Arc::new(Fixed)),
        ..ReadToolOptions::default()
    };
    let mut definition = create_read_tool_definition::<()>("/work".into(), options);
    let render_call = definition.render_call.take().unwrap();
    let executable = wrap_tool_definition(definition, None);
    let args = json!({"file_path": "alias.txt", "offset": 2});
    let ctx = context(&args, &setup(), &env, None);
    let row = render_call(&args, &env.theme, &ctx).unwrap();
    assert_eq!(
        strip_ansi(&row.render(WIDTH)[0]).unwrap().trim_end(),
        "read alias.txt:2"
    );
    let rejected = (executable.execute)("call".into(), args, None, None)
        .now_or_never()
        .unwrap()
        .err()
        .unwrap();
    assert!(rejected.message.contains("path"), "{}", rejected.message);
    let served = (executable.execute)("call".into(), json!({"path": "alias.txt"}), None, None)
        .now_or_never()
        .unwrap()
        .unwrap();
    let UserBlock::Text(text) = &served.content[0] else {
        panic!("expected text content");
    };
    assert_eq!(text.text, "served");
}

#[test]
fn one_read_definition_follows_each_render_context() {
    let _bindings = bind(&["ctrl+o".to_owned()]);
    let env = Env::new(0);
    let definition = tool();
    let args = json!({"path": "docs/guide.md"});
    let title = |cwd: &str, readme: &str| {
        let mut setup = setup();
        setup.expanded = false;
        setup.cwd = cwd.to_owned();
        setup.readme = readme.to_owned();
        let ctx = context(&args, &setup, &env, None);
        let row = (definition.render_call.as_ref().unwrap())(&args, &env.theme, &ctx).unwrap();
        strip_ansi(&row.render(WIDTH)[0])
            .unwrap()
            .trim_end()
            .to_owned()
    };
    assert_eq!(
        title("/opt/maestro", "/opt/maestro/README.md"),
        "read docs docs/guide.md (ctrl+o to expand)"
    );
    assert_eq!(
        title("/work", "/opt/maestro/README.md"),
        "read docs/guide.md"
    );
    assert_eq!(
        title("/opt/maestro", "/srv/other/README.md"),
        "read docs/guide.md"
    );
    assert_eq!(
        title("/opt/maestro", "/opt/maestro/README.md"),
        "read docs docs/guide.md (ctrl+o to expand)"
    );
}

#[cfg(windows)]
#[test]
fn rooted_windows_documentation_paths_take_the_process_drive() {
    let _bindings = bind(&["ctrl+o".to_owned()]);
    let env = Env::new(0);
    let process = std::env::current_dir().unwrap();
    let drive = process.to_string_lossy()[..2].to_owned();
    let other = if drive.eq_ignore_ascii_case("D:") {
        "E:"
    } else {
        "D:"
    };
    let mut setup = setup();
    setup.expanded = false;
    setup.readme = format!("{drive}\\pkg\\README.md");
    setup.cwd = format!("{other}\\project");
    let args = json!({"path": "\\pkg\\docs\\guide.md"});
    let ctx = context(&args, &setup, &env, None);
    let row = (tool().render_call.as_ref().unwrap())(&args, &env.theme, &ctx).unwrap();
    let text = strip_ansi(&row.render(WIDTH)[0]).unwrap();
    assert!(text.starts_with("read docs docs/guide.md"), "{text}");
}

#[cfg(windows)]
#[test]
fn documentation_paths_respect_windows_roots_and_separators() {
    let env = Env::new(0);
    for case in cases("documentation_paths_respect_windows_roots_and_separators") {
        let mut setup = setup();
        setup.readme = case.input["readme"].as_str().unwrap().to_owned();
        setup.expanded = false;
        let args = json!({"path": case.input["absolutePath"]});
        let ctx = context(&args, &setup, &env, None);
        let row = (tool().render_call.as_ref().unwrap())(&args, &env.theme, &ctx).unwrap();
        let text = strip_ansi(&row.render(WIDTH)[0]).unwrap();
        match case.expected.get("label") {
            Some(label) => assert!(
                text.starts_with(&format!("read docs {}", label.as_str().unwrap())),
                "case {}",
                case.id
            ),
            None => assert!(!text.starts_with("read docs"), "case {}", case.id),
        }
    }
}

#[cfg(unix)]
#[test]
fn classification_reads_the_process_directory_only_for_operands_that_need_it() {
    const CHILD: &str = "MAESTRO_READ_RENDER_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "classification_reads_the_process_directory_only_for_operands_that_need_it",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let parent = std::env::temp_dir().join(format!("maestro-render-gone-{}", std::process::id()));
    let gone = parent.join("gone");
    std::fs::create_dir_all(&gone).unwrap();
    std::env::set_current_dir(&gone).unwrap();
    std::fs::remove_dir(&gone).unwrap();
    std::fs::remove_dir(&parent).unwrap();
    assert!(std::env::current_dir().is_err());
    read_renders_without_process_directory();
}

/// Renders read calls and results while the process directory is unavailable.
#[cfg(unix)]
fn read_renders_without_process_directory() {
    let _bindings = bind(&["ctrl+o".to_owned()]);
    let env = Env::new(0);
    let title = |path: &str, readme: &str| {
        let args = json!({"path": path});
        let mut setup = setup();
        setup.expanded = false;
        setup.readme = readme.to_owned();
        let ctx = context(&args, &setup, &env, None);
        (tool().render_call.as_ref().unwrap())(&args, &env.theme, &ctx).map(|row| {
            strip_ansi(&row.render(WIDTH)[0])
                .unwrap()
                .trim_end()
                .to_owned()
        })
    };
    let readme = "/opt/maestro/README.md";
    assert_eq!(
        title("/opt/maestro/docs/guide.md", readme).unwrap(),
        "read docs docs/guide.md (ctrl+o to expand)"
    );
    assert_eq!(
        title("/opt/maestro/README.md", readme).unwrap(),
        "read docs README.md (ctrl+o to expand)"
    );
    assert_eq!(
        title("/srv/other/AGENTS.md", readme).unwrap(),
        "read resource AGENTS.md (ctrl+o to expand)"
    );
    assert!(title("/opt/maestro/docs/guide.md", "relative/README.md").is_err());
    read_results_without_process_directory(&env);
}

/// Renders read results while the process directory is unavailable.
#[cfg(unix)]
fn read_results_without_process_directory(env: &Env) {
    let readme = "/opt/maestro/README.md";
    let result = |path: &str, readme: &str| {
        let args = json!({"path": path});
        let mut setup = setup();
        setup.expanded = false;
        setup.readme = readme.to_owned();
        let ctx = context(&args, &setup, env, None);
        (tool().render_result.as_ref().unwrap())(
            &text_result("served"),
            settled(false),
            &env.theme,
            &ctx,
        )
        .map(|row| {
            row.render(WIDTH)
                .iter()
                .map(|line| strip_ansi(line).unwrap())
                .collect::<String>()
        })
    };
    for hidden in [
        "/opt/maestro/docs/guide.md",
        "/opt/maestro/README.md",
        "/srv/other/AGENTS.md",
    ] {
        assert!(
            !result(hidden, readme).unwrap().contains("served"),
            "{hidden}"
        );
    }
    assert!(
        result("/srv/plain/notes.txt", readme)
            .unwrap()
            .contains("served")
    );
    assert!(result("/opt/maestro/docs/guide.md", "relative/README.md").is_err());
}
