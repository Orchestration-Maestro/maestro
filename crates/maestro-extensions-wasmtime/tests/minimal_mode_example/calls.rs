//! Calls through the registered renderer, including partial operands and home abbreviation.
use super::{expected, fg, loaded, run, theme};
use maestro_extensions::{ExtensionRunner, Local};
use maestro_test_directory::Scratch;
use serde_json::{Value, json};
use std::rc::Rc;

/// Renders one registered call with the original row environment.
fn call(runner: &ExtensionRunner<Local>, name: &str, args: &Value) -> Vec<String> {
    let tool = runner
        .get_all_registered_tools()
        .into_iter()
        .find(|tool| tool.definition.name() == name)
        .unwrap();
    let rows = maestro_extensions::ToolRendererState::default();
    let redraws = Rc::new(std::cell::Cell::new(0));
    let context = super::super::canonical_ui::row_context(args, &rows, &redraws);
    tool.definition
        .renderer_binding()
        .unwrap()
        .render_call
        .unwrap()(Some(args), &theme(), &context)
    .unwrap()
    .try_render(200)
    .unwrap()
}

/// An independent expected title, with native bold formatting.
fn title(text: &str) -> String {
    fg(
        &maestro_theme::ThemeColor::ToolTitle,
        &maestro_theme::Theme::bold(text, true),
    )
}

/// Checks authored call literals through the registered renderer.
fn check_calls(runner: &ExtensionRunner<Local>, cases: Vec<(&str, Value, String)>) {
    for (name, args, literal) in cases {
        assert_eq!(
            call(runner, name, &args),
            expected(literal),
            "{name} {args}"
        );
    }
}

/// Recorded read call operands and defaults.
fn read_cases(home: &str) -> Vec<(&'static str, Value, String)> {
    vec![
        (
            "read",
            json!({}),
            format!(
                "{} {}",
                title("read"),
                fg(&maestro_theme::ThemeColor::ToolOutput, "...")
            ),
        ),
        (
            "read",
            json!({"path":format!("{home}/src.rs"),"offset":3,"limit":2}),
            format!(
                "{} {}{}",
                title("read"),
                fg(&maestro_theme::ThemeColor::Accent, "~/src.rs"),
                fg(&maestro_theme::ThemeColor::Warning, ":3-4")
            ),
        ),
        (
            "read",
            json!({"path":"x","limit":2}),
            format!(
                "{} {}{}",
                title("read"),
                fg(&maestro_theme::ThemeColor::Accent, "x"),
                fg(&maestro_theme::ThemeColor::Warning, ":1-2")
            ),
        ),
        (
            "read",
            json!({"limit":0}),
            format!(
                "{} {}{}",
                title("read"),
                fg(&maestro_theme::ThemeColor::ToolOutput, "..."),
                fg(&maestro_theme::ThemeColor::Warning, ":1")
            ),
        ),
        (
            "read",
            json!({"offset":4}),
            format!(
                "{} {}{}",
                title("read"),
                fg(&maestro_theme::ThemeColor::ToolOutput, "..."),
                fg(&maestro_theme::ThemeColor::Warning, ":4")
            ),
        ),
    ]
}

/// Recorded bash call operands and defaults.
fn bash_cases() -> Vec<(&'static str, Value, String)> {
    vec![
        ("bash", json!({}), title("$ ...")),
        (
            "bash",
            json!({"command":"printf hi","timeout":0}),
            title("$ printf hi"),
        ),
        (
            "bash",
            json!({"command":"printf hi","timeout":5}),
            format!(
                "{}{}",
                title("$ printf hi"),
                fg(&maestro_theme::ThemeColor::Muted, " (timeout 5s)")
            ),
        ),
    ]
}

/// Recorded file call operands and defaults.
fn file_cases() -> Vec<(&'static str, Value, String)> {
    vec![
        (
            "write",
            json!({}),
            format!(
                "{} {}",
                title("write"),
                fg(&maestro_theme::ThemeColor::ToolOutput, "...")
            ),
        ),
        (
            "write",
            json!({"path":"x","content":"a\nb\n"}),
            format!(
                "{} {}{}",
                title("write"),
                fg(&maestro_theme::ThemeColor::Accent, "x"),
                fg(&maestro_theme::ThemeColor::Muted, " (3 lines)")
            ),
        ),
        (
            "write",
            json!({"path":"x","content":""}),
            format!(
                "{} {}",
                title("write"),
                fg(&maestro_theme::ThemeColor::Accent, "x")
            ),
        ),
        (
            "edit",
            json!({}),
            format!(
                "{} {}",
                title("edit"),
                fg(&maestro_theme::ThemeColor::ToolOutput, "...")
            ),
        ),
        (
            "edit",
            json!({"path":"x"}),
            format!(
                "{} {}",
                title("edit"),
                fg(&maestro_theme::ThemeColor::Accent, "x")
            ),
        ),
    ]
}

/// Recorded search call operands and defaults.
fn search_cases() -> Vec<(&'static str, Value, String)> {
    vec![
        (
            "find",
            json!({}),
            format!(
                "{} {}{}",
                title("find"),
                fg(&maestro_theme::ThemeColor::Accent, ""),
                fg(&maestro_theme::ThemeColor::ToolOutput, " in .")
            ),
        ),
        (
            "find",
            json!({"pattern":"*.rs","path":"src","limit":0}),
            format!(
                "{} {}{}{}",
                title("find"),
                fg(&maestro_theme::ThemeColor::Accent, "*.rs"),
                fg(&maestro_theme::ThemeColor::ToolOutput, " in src"),
                fg(&maestro_theme::ThemeColor::ToolOutput, " (limit 0)")
            ),
        ),
        (
            "grep",
            json!({}),
            format!(
                "{} {}{}",
                title("grep"),
                fg(&maestro_theme::ThemeColor::Accent, "//"),
                fg(&maestro_theme::ThemeColor::ToolOutput, " in .")
            ),
        ),
        (
            "grep",
            json!({"pattern":"needle","path":"src","glob":"*.rs","limit":2}),
            format!(
                "{} {}{}{}{}",
                title("grep"),
                fg(&maestro_theme::ThemeColor::Accent, "/needle/"),
                fg(&maestro_theme::ThemeColor::ToolOutput, " in src"),
                fg(&maestro_theme::ThemeColor::ToolOutput, " (*.rs)"),
                fg(&maestro_theme::ThemeColor::ToolOutput, " limit 2")
            ),
        ),
    ]
}

/// Recorded listing call operands and defaults.
fn listing_cases() -> Vec<(&'static str, Value, String)> {
    vec![
        (
            "ls",
            json!({}),
            format!(
                "{} {}",
                title("ls"),
                fg(&maestro_theme::ThemeColor::Accent, ".")
            ),
        ),
        (
            "ls",
            json!({"path":"src","limit":0}),
            format!(
                "{} {}{}",
                title("ls"),
                fg(&maestro_theme::ThemeColor::Accent, "src"),
                fg(&maestro_theme::ThemeColor::ToolOutput, " (limit 0)")
            ),
        ),
    ]
}

#[test]
fn minimal_mode_renders_all_call_operands_and_defaults() {
    run(async {
        let root = Scratch::new().unwrap();
        let runner = loaded(root.path()).await;
        check_calls(&runner, read_cases(&std::env::var("HOME").unwrap()));
        check_calls(&runner, bash_cases());
        check_calls(&runner, file_cases());
        check_calls(&runner, search_cases());
        check_calls(&runner, listing_cases());
    });
}

#[test]
fn minimal_mode_shortens_only_the_home_directory() {
    use maestro_theme::{Theme, ThemeColor};
    run(async {
        let root = Scratch::new().unwrap();
        let runner = loaded(root.path()).await;
        let home = std::env::var("HOME").unwrap();
        let theme = theme().get().unwrap();
        let title = theme
            .fg(&ThemeColor::ToolTitle, &Theme::bold("read", true))
            .unwrap();
        for (path, shown) in [
            (home.clone(), "~".into()),
            (format!("{home}/src.rs"), "~/src.rs".into()),
            (
                format!(r"{home}\sibling/src.rs"),
                format!(r"{home}\sibling/src.rs"),
            ),
            (
                format!("{home}-other/src.rs"),
                format!("{home}-other/src.rs"),
            ),
        ] {
            let literal = format!("{title} {}", theme.fg(&ThemeColor::Accent, &shown).unwrap());
            assert_eq!(
                call(&runner, "read", &json!({"path":path})),
                expected(literal)
            );
        }
    });
}
