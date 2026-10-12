//! Recorded display cases, a controlled theme and keybinding registry, and the call and result drivers.
#![cfg(test)]
#![allow(dead_code)] // each test binary uses a different part of these helpers

use maestro_agent::AgentToolResult;
use maestro_models::{DiagnosticErrorInfo, UserBlock};
use maestro_theme::{
    ColorMode, ColorValue, LiveTheme, NativeThemeOperations, SyntaxHighlighter, SyntectHighlighter,
    Theme, ThemeColor, ThemeDirectories, ThemeOptions, ThemeState,
};
use maestro_tools::output::render_utils::strip_ansi;
use maestro_tools::output::truncate::{
    DEFAULT_MAX_BYTES, DEFAULT_MAX_LINES, TruncationBy, TruncationResult,
};
use maestro_tools::{
    ReadToolDetails, ToolRenderContext, ToolRenderPhase, ToolRenderResultOptions,
    create_read_tool_definition,
};
use maestro_tui::{
    ImageProtocol, KeybindingDefinition, KeybindingKeys, KeybindingsManager, TerminalCapabilities,
    TerminalImage, get_keybindings, set_keybindings, tui::ComponentHandle, visible_width,
};
use serde::Deserialize;
use serde_json::Value;
use std::{
    cell::RefCell,
    collections::HashSet,
    rc::Rc,
    sync::{Mutex, MutexGuard, PoisonError},
};

/// Viewport width every recorded row was laid out for.
pub const WIDTH: usize = 1000;

/// Fallible public test observation.
pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// One recorded invocation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// Owning public test.
    pub test: String,
    /// Distinguishable case label.
    pub id: String,
    /// Operation selected at the public boundary.
    pub op: String,
    /// Operation-specific input.
    pub input: Value,
    /// Independently recorded expectation.
    pub expected: Value,
}

/// The committed corpus.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Theme keys, whose index plus one is each key's palette color.
    theme_keys: Vec<String>,
    /// Recorded invocations.
    cases: Vec<Case>,
}

/// Input members every case may carry; reading one not listed here fails the test.
const MEMBERS: [&str; 13] = [
    "args",
    "context",
    "result",
    "options",
    "text",
    "value",
    "width",
    "absolutePath",
    "readme",
    "path",
    "home",
    "binding",
    "slot",
];

/// Context members a case may carry.
const CONTEXT_MEMBERS: [&str; 10] = [
    "readme",
    "bindings",
    "expanded",
    "images",
    "isError",
    "showImages",
    "cwd",
    "executionStarted",
    "argsComplete",
    "themeShift",
];

/// The theme keys of the corpus.
pub fn theme_keys() -> Vec<String> {
    let corpus: Corpus =
        serde_json::from_str(include_str!("../fixtures/read_display.json")).unwrap();
    corpus.theme_keys
}

/// The nonempty group of one test, after rejecting repeated queries and unknown members.
pub fn cases(test: &str) -> Vec<Case> {
    let corpus: Corpus =
        serde_json::from_str(include_str!("../fixtures/read_display.json")).unwrap();
    let mut queries = HashSet::new();
    for case in &corpus.cases {
        assert!(
            queries.insert((&case.op, case.input.to_string())),
            "duplicate query {}",
            case.id
        );
        let input = case.input.as_object().unwrap();
        assert!(
            input.keys().all(|key| MEMBERS.contains(&key.as_str())),
            "unknown input member"
        );
        if let Some(context) = input.get("context") {
            let context = context.as_object().unwrap();
            assert!(
                context
                    .keys()
                    .all(|key| CONTEXT_MEMBERS.contains(&key.as_str())),
                "unknown context member"
            );
        }
    }
    let selected: Vec<_> = corpus
        .cases
        .into_iter()
        .filter(|case| case.test == test)
        .collect();
    assert!(!selected.is_empty(), "empty test group {test}");
    selected
}

/// A theme whose key `i` of the corpus has palette color `i + 1 + shift`.
pub fn palette(shift: u8) -> Theme {
    let keys = theme_keys();
    let colors = keys.iter().enumerate().map(|(index, key)| {
        let color = ThemeColor::Named(key.clone());
        (
            color,
            ColorValue::Index(u8::try_from(index + 1).unwrap() + shift),
        )
    });
    Theme::new(colors, [], ColorMode::Color256, ThemeOptions::default()).unwrap()
}

/// A theme with no colors.
pub fn bare_theme() -> Theme {
    Theme::new([], [], ColorMode::Color256, ThemeOptions::default()).unwrap()
}

/// Serializes the process-wide keybinding registry and restores it afterwards.
pub struct Bindings {
    /// Held until the registry is restored.
    _lock: MutexGuard<'static, ()>,
    /// The registry installed before.
    previous: KeybindingsManager,
}

/// Guards the process-wide keybinding registry.
static REGISTRY: Mutex<()> = Mutex::new(());

/// Install a registry binding the expand action to `keys`.
pub fn bind(keys: &[String]) -> Bindings {
    let lock = REGISTRY.lock().unwrap_or_else(PoisonError::into_inner);
    let previous = get_keybindings();
    let definition = KeybindingDefinition {
        default_keys: KeybindingKeys::Multiple(keys.to_vec()),
        description: None,
    };
    set_keybindings(KeybindingsManager::new(
        vec![("app.tools.expand".into(), definition)],
        Vec::new(),
    ));
    Bindings {
        _lock: lock,
        previous,
    }
}

impl Drop for Bindings {
    fn drop(&mut self) {
        set_keybindings(self.previous.clone());
    }
}

/// The collaborators a renderer receives besides its arguments.
pub struct Env {
    /// Owner of the published theme.
    pub state: Rc<ThemeState>,
    /// The published theme's handle.
    pub live: LiveTheme,
    /// The theme the renderers are given.
    pub theme: Rc<Theme>,
    /// Classifies code.
    pub highlighter: Rc<dyn SyntaxHighlighter>,
    /// Image support of the terminal.
    pub terminal: TerminalImage,
}

impl Env {
    /// A published palette theme shifted by `shift`, the native highlighter and no image support.
    pub fn new(shift: u8) -> Self {
        thread_local! {
            /// Grammars load once per test thread.
            static NATIVE: Rc<SyntectHighlighter> = Rc::new(SyntectHighlighter::new().unwrap());
        }
        Self::with(Rc::new(palette(shift)), NATIVE.with(Rc::clone))
    }

    /// An environment publishing `theme` and highlighting with `highlighter`.
    pub fn with(theme: Rc<Theme>, highlighter: Rc<dyn SyntaxHighlighter>) -> Self {
        let state = Rc::new(ThemeState::new(
            ThemeDirectories {
                themes_dir: String::new(),
                custom_themes_dir: String::new(),
            },
            Rc::new(NativeThemeOperations),
        ));
        state.set_theme_instance(Rc::clone(&theme)).unwrap();
        let live = state.theme();
        Self {
            state,
            live,
            theme,
            highlighter,
            terminal: TerminalImage::new(|_| None, || 1),
        }
    }

    /// Publish a palette shifted by `shift` and make it the renderers' theme.
    pub fn republish(&mut self, shift: u8) {
        self.theme = Rc::new(palette(shift));
        self.state
            .set_theme_instance(Rc::clone(&self.theme))
            .unwrap();
    }
}

/// Everything a recorded context says.
pub struct Setup {
    /// Absolute documentation README.
    pub readme: String,
    /// Keys bound to the expand action.
    pub bindings: Vec<String>,
    /// Whether the row is expanded.
    pub expanded: bool,
    /// Working directory.
    pub cwd: String,
    /// Image protocol of the terminal.
    pub images: Option<ImageProtocol>,
    /// Whether images are shown.
    pub show_images: bool,
    /// Whether the result is an error.
    pub is_error: bool,
    /// Execution progress.
    pub phase: ToolRenderPhase,
    /// Palette shift of the published theme.
    pub shift: u8,
}

impl Setup {
    /// Decode a recorded context object.
    pub fn from(context: &Value) -> Self {
        let flag = |name: &str| context.get(name).and_then(Value::as_bool).unwrap_or(false);
        let images = match context.get("images").and_then(Value::as_str) {
            Some("kitty") => Some(ImageProtocol::Kitty),
            Some("iterm2") => Some(ImageProtocol::Iterm2),
            Some(other) => panic!("unknown image protocol {other}"),
            None => None,
        };
        Self {
            readme: context["readme"].as_str().unwrap().to_owned(),
            bindings: context["bindings"]
                .as_array()
                .unwrap()
                .iter()
                .map(|key| key.as_str().unwrap().to_owned())
                .collect(),
            expanded: flag("expanded"),
            cwd: context
                .get("cwd")
                .and_then(Value::as_str)
                .unwrap_or("/work")
                .to_owned(),
            images,
            show_images: context
                .get("showImages")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            is_error: flag("isError"),
            phase: ToolRenderPhase {
                execution_started: flag("executionStarted"),
                args_complete: flag("argsComplete"),
                is_partial: false,
            },
            shift: context
                .get("themeShift")
                .and_then(Value::as_u64)
                .map_or(0, |shift| u8::try_from(shift).unwrap()),
        }
    }
}

/// A render context over `args` for the recorded setup.
pub fn context<'a>(
    args: &'a Value,
    setup: &Setup,
    env: &Env,
    last: Option<ComponentHandle>,
) -> ToolRenderContext<'a> {
    env.terminal.set_capabilities(TerminalCapabilities {
        images: setup.images,
        true_color: true,
        hyperlinks: true,
    });
    ToolRenderContext {
        args,
        tool_call_id: "call".into(),
        invalidate: Rc::new(|| {}),
        last_component: last,
        state: Rc::new(RefCell::new(Value::Null)),
        cwd: setup.cwd.clone(),
        phase: setup.phase,
        expanded: setup.expanded,
        show_images: setup.show_images,
        is_error: setup.is_error,
        readme_path: setup.readme.clone(),
        live_theme: env.live.clone(),
        highlighter: Rc::clone(&env.highlighter),
        terminal_image: env.terminal.clone(),
    }
}

/// Recorded truncation fields.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RecordedTruncation {
    /// Whether a limit cut the text.
    truncated: bool,
    /// Stopping limit.
    truncated_by: String,
    /// Selected entries.
    output_lines: usize,
    /// Original entries.
    total_lines: usize,
    /// Applied entry budget; absent means the default.
    max_lines: Option<usize>,
    /// Applied byte budget; absent means the default.
    max_bytes: Option<usize>,
    /// Whether the first entry exceeded the byte budget.
    #[serde(default)]
    first_line_exceeds_limit: bool,
}

/// A recorded result object as the read tool's typed result.
pub fn read_result(result: &Value) -> AgentToolResult<ReadToolDetails> {
    let content: Vec<UserBlock> = serde_json::from_value(result["content"].clone()).unwrap();
    let truncation = result.get("details").map(|details| {
        let recorded: RecordedTruncation =
            serde_json::from_value(details["truncation"].clone()).unwrap();
        TruncationResult {
            content: String::new(),
            truncated: recorded.truncated,
            truncated_by: Some(if recorded.truncated_by == "lines" {
                TruncationBy::Lines
            } else {
                TruncationBy::Bytes
            }),
            total_lines: recorded.total_lines,
            total_bytes: 0,
            output_lines: recorded.output_lines,
            output_bytes: 0,
            last_line_partial: false,
            first_line_exceeds_limit: recorded.first_line_exceeds_limit,
            max_lines: recorded.max_lines.unwrap_or(DEFAULT_MAX_LINES),
            max_bytes: recorded.max_bytes.unwrap_or(DEFAULT_MAX_BYTES),
        }
    });
    AgentToolResult {
        content,
        details: truncation.map(|truncation| ReadToolDetails {
            truncation: Some(truncation),
        }),
        terminate: None,
    }
}

/// Result options of a recorded case.
pub fn options(options: &Value) -> ToolRenderResultOptions {
    ToolRenderResultOptions {
        expanded: options["expanded"].as_bool().unwrap(),
        is_partial: options["isPartial"].as_bool().unwrap(),
    }
}

/// Render a recorded call or result and return its rows.
pub fn render_case(case: &Case) -> Vec<String> {
    let setup = Setup::from(&case.input["context"]);
    let _bindings = bind(&setup.bindings);
    let env = Env::new(setup.shift);
    let tool = create_read_tool_definition::<()>(
        "/work".into(),
        maestro_tools::ReadToolOptions::default(),
    );
    let args = &case.input["args"];
    let ctx = context(args, &setup, &env, None);
    let row = if case.op == "call" {
        (tool.render_call.as_ref().unwrap())(args, &env.theme, &ctx)
    } else {
        let result = read_result(&case.input["result"]);
        let options = options(&case.input["options"]);
        (tool.render_result.as_ref().unwrap())(&result, options, &env.theme, &ctx)
    };
    row.unwrap().render(WIDTH)
}

/// The recorded rows padded to the viewport.
pub fn padded(expected: &str) -> String {
    format!("{expected}{}", " ".repeat(WIDTH - visible_width(expected)))
}

/// A recorded row made only of style escapes shown as an empty row, which is how the toolkit's
/// wrapping lays out a line without text.
pub fn unstyled_blank(row: &str) -> String {
    if strip_ansi(row).unwrap().is_empty() {
        String::new()
    } else {
        row.to_owned()
    }
}

/// Assert that `actual` rows equal the recorded `expected` rows once padded.
pub fn assert_rows(case: &Case, actual: &[String], plain: bool) {
    let expected: Vec<String> = case
        .expected
        .as_array()
        .unwrap()
        .iter()
        .map(|row| padded(&unstyled_blank(row.as_str().unwrap())))
        .collect();
    let actual: Vec<String> = if plain {
        actual
            .iter()
            .map(|row| maestro_tools::output::render_utils::strip_ansi(row).unwrap())
            .collect()
    } else {
        actual.to_vec()
    };
    assert_eq!(actual, expected, "case {}", case.id);
}

/// Run every recorded case of `test`, comparing colors unless `plain`.
pub fn check_rows(test: &str, plain: bool) {
    for case in cases(test) {
        let actual = render_case(&case);
        assert_rows(&case, &actual, plain);
    }
}

/// A failure message, for assertions over error results.
pub fn message(error: &DiagnosticErrorInfo) -> &str {
    &error.message
}
