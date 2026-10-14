//! Recorded display cases, a controlled theme and keybinding registry, and the call and result drivers.
#![cfg(test)]
#![allow(dead_code)] // each test binary uses a different part of these helpers

#[path = "../../../maestro-test-terminal/tests/support/manual_runtime.rs"]
pub mod manual_runtime;

use maestro_agent::AgentToolResult;
use maestro_models::UserBlock;
use maestro_theme::{
    ColorMode, ColorValue, LiveTheme, NativeThemeOperations, SyntaxHighlighter, SyntaxSpan,
    SyntectHighlighter, Theme, ThemeColor, ThemeDirectories, ThemeOptions, ThemeState,
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
    error::Error,
    rc::Rc,
    sync::{Mutex, MutexGuard, PoisonError},
};

/// A highlighter that supports no language.
pub struct NoLanguages;

impl SyntaxHighlighter for NoLanguages {
    fn supports_language(&self, _: &str) -> bool {
        false
    }

    fn highlight(&self, _: &str, _: &str) -> Result<Vec<SyntaxSpan>, Box<dyn Error + Send + Sync>> {
        Ok(Vec::new())
    }
}

/// Viewport width every recorded row was laid out for.
pub const WIDTH: usize = 1000;

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

/// Select a nonempty search group after validating the corpus and query identities.
pub fn search_cases(source: &str, test: &str) -> Vec<Case> {
    let corpus: Corpus = serde_json::from_str(source).unwrap();
    let mut seen = HashSet::new();
    for case in &corpus.cases {
        assert!(
            seen.insert((&case.test, &case.op, case.input.to_string())),
            "duplicate query"
        );
    }
    assert_eq!(corpus.theme_keys.len(), 7);
    let selected: Vec<_> = corpus
        .cases
        .into_iter()
        .filter(|case| case.test == test)
        .collect();
    assert!(!selected.is_empty(), "missing group {test}");
    selected
}

/// Return the recorded search palette values in their supplied order.
pub fn search_keys(source: &str) -> Vec<String> {
    serde_json::from_str::<Corpus>(source).unwrap().theme_keys
}

/// Every (test, operation) group the corpus may hold; a case outside them is never run.
const GROUPS: [(&str, &str); 31] = [
    (
        "read_titles_keep_argument_precedence_and_invalid_styles",
        "call",
    ),
    (
        "read_line_ranges_preserve_numeric_spelling_and_presence",
        "call",
    ),
    (
        "read_classification_obeys_skill_docs_resource_priority",
        "call",
    ),
    (
        "classified_reads_hide_only_successful_compact_results",
        "result",
    ),
    ("expanded_read_titles_return_to_authored_paths", "call"),
    ("expanded_read_results_reveal_classified_content", "result"),
    ("read_errors_bypass_compact_hiding", "result"),
    ("compact_read_ranges_precede_live_expand_hints", "call"),
    (
        "read_preview_limits_count_logical_lines_before_wrapping",
        "result",
    ),
    (
        "read_trailing_empty_rows_do_not_trim_visible_whitespace",
        "result",
    ),
    (
        "read_truncation_notices_follow_reason_and_explicit_limits",
        "result",
    ),
    (
        "read_highlighting_uses_only_selected_filename_language",
        "result",
    ),
    (
        "read_image_fallback_obeys_capability_and_visibility",
        "result",
    ),
    (
        "read_image_indicators_keep_order_and_dimension_fallbacks",
        "result",
    ),
    (
        "display_stripping_matches_complete_escape_sequences",
        "strip",
    ),
    ("shell_sanitization_keeps_only_allowed_controls", "sanitize"),
    (
        "read_output_sanitizes_before_tab_expansion_and_layout",
        "result",
    ),
    (
        "display_string_selection_distinguishes_absent_and_wrong_types",
        "str",
    ),
    (
        "display_literal_transforms_preserve_other_whitespace",
        "tabs",
    ),
    ("display_literal_transforms_preserve_other_whitespace", "cr"),
    (
        "display_paths_abbreviate_only_home_and_descendants",
        "shorten",
    ),
    (
        "key_hints_use_current_bindings_order_and_separate_styles",
        "keys",
    ),
    ("display_width_cases_use_shared_toolkit_boundaries", "width"),
    ("read_renderers_reuse_the_supplied_text_slot", "reuse"),
    (
        "display_output_handles_absence_and_orders_content_by_kind",
        "output",
    ),
    (
        "read_context_uses_current_cwd_and_documentation_path",
        "call",
    ),
    (
        "read_renderer_errors_leave_previous_text_unchanged",
        "error",
    ),
    (
        "classified_results_avoid_unused_display_dependencies",
        "early_return",
    ),
    (
        "read_highlighting_reads_the_current_published_theme",
        "result",
    ),
    (
        "read_result_uses_options_expansion_not_unrelated_flags",
        "result",
    ),
    (
        "documentation_paths_respect_windows_roots_and_separators",
        "classification_windows",
    ),
];

/// Input members every case may carry; reading one not listed here fails the test.
const MEMBERS: [&str; 12] = [
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
            GROUPS.contains(&(case.test.as_str(), case.op.as_str())),
            "case {} is in no test group",
            case.id
        );
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

/// A theme whose key `i` of the read corpus has palette color `i + 1 + shift`.
pub fn palette(shift: u8) -> Theme {
    palette_of(&theme_keys(), shift)
}

/// A theme whose key `i` of `keys` has palette color `i + 1 + shift`.
pub fn palette_of(keys: &[String], shift: u8) -> Theme {
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

/// The native highlighter, whose grammars load once per test thread.
pub fn native_highlighter() -> Rc<SyntectHighlighter> {
    thread_local! {
        /// Grammars load once per test thread.
        static NATIVE: Rc<SyntectHighlighter> = Rc::new(SyntectHighlighter::new().unwrap());
    }
    NATIVE.with(Rc::clone)
}

/// Records every highlight request and classifies the whole text as a keyword.
pub struct Probe {
    /// Requests as (code, language).
    pub calls: RefCell<Vec<(String, String)>>,
    /// Whether the engine claims every language.
    supported: bool,
}

impl Probe {
    /// A probe that claims every language when `supported`.
    pub fn new(supported: bool) -> Rc<Self> {
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
    /// Controlled terminal runtime.
    pub runtime: Rc<manual_runtime::ManualRuntime>,
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
        Self::with(Rc::new(palette(shift)), native_highlighter())
    }

    /// Like [`Env::new`] with the palette built from `keys`.
    pub fn for_keys(keys: &[String], shift: u8) -> Self {
        Self::with(Rc::new(palette_of(keys, shift)), native_highlighter())
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
            runtime: Rc::new(manual_runtime::ManualRuntime::default()),
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
pub fn context<'a, S: Default>(
    args: &'a Value,
    setup: &Setup,
    env: &Env,
    last: Option<ComponentHandle>,
) -> ToolRenderContext<'a, S> {
    env.terminal.set_capabilities(TerminalCapabilities {
        images: setup.images,
        true_color: true,
        hyperlinks: true,
    });
    ToolRenderContext {
        args: Some(args),
        tool_call_id: "call".into(),
        invalidate: Rc::new(|| {}),
        last_component: last,
        state: Rc::new(RefCell::new(S::default())),
        cwd: setup.cwd.clone(),
        phase: setup.phase,
        expanded: setup.expanded,
        show_images: setup.show_images,
        is_error: setup.is_error,
        environment: Rc::new(maestro_tools::ToolRenderEnvironment {
            runtime: env.runtime.clone(),
            readme_path: setup.readme.clone(),
            live_theme: env.live.clone(),
            highlighter: Rc::clone(&env.highlighter),
            terminal_image: env.terminal.clone(),
        }),
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
        (tool.render_call.as_ref().unwrap())(Some(args), &env.theme, &ctx)
    } else {
        let result = read_result(&case.input["result"]);
        let options = options(&case.input["options"]);
        (tool.render_result.as_ref().unwrap())(
            &maestro_tools::output::render_utils::ToolRenderResultLike::from(&result),
            options,
            &env.theme,
            &ctx,
        )
    };
    row.unwrap().render(WIDTH)
}

/// The recorded rows padded to the viewport.
pub fn padded(expected: &str) -> String {
    padded_to(expected, WIDTH)
}

/// The recorded row padded to `width` columns.
pub fn padded_to(expected: &str, width: usize) -> String {
    format!("{expected}{}", " ".repeat(width - visible_width(expected)))
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

/// Load the shipped theme `name`, optionally selecting its color mode.
pub fn shipped_theme(name: &str, mode: Option<ColorMode>) -> Rc<Theme> {
    let path = format!(
        "{}/../maestro-theme/assets/theme/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    Rc::new(maestro_theme::load_theme_from_path(&path, mode, &NativeThemeOperations).unwrap())
}
