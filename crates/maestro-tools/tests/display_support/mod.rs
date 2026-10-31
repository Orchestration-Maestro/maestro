//! Recorded display cases, a controlled keybinding registry and the call and result drivers.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

pub use crate::display_cases::*;
pub use crate::display_context::*;

use maestro_agent::AgentToolResult;
use maestro_models::UserBlock;
use maestro_theme::{
    ColorMode, LiveTheme, SyntaxHighlighter, SyntaxSpan, Theme, ThemeColor, ThemeOptions,
};
use maestro_tools::output::render_utils::strip_ansi;
use maestro_tools::output::truncate::{
    DEFAULT_MAX_BYTES, DEFAULT_MAX_LINES, TruncationBy, TruncationResult,
};
use maestro_tools::{
    ReadToolDetails, ToolRenderContext, ToolRenderResultOptions, create_read_tool_definition,
};
use maestro_tui::{
    KeybindingDefinition, KeybindingKeys, KeybindingsManager, get_keybindings, set_keybindings,
    tui::ComponentHandle, visible_width,
};
use serde::Deserialize;
use serde_json::Value;
use std::{
    cell::RefCell,
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

/// A theme with no colors.
pub fn bare_theme() -> Theme {
    Theme::new([], [], ColorMode::Color256, ThemeOptions::default()).unwrap()
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

impl Env {
    /// Like [`Env::new`] with the palette built from `keys`.
    pub fn for_keys(keys: &[String], shift: u8) -> Self {
        Self::with(Rc::new(palette_of(keys, shift)), native_highlighter())
    }
    /// Publish a palette shifted by `shift` and make it the renderers' theme.
    pub fn republish(&self, shift: u8) {
        self.publish(Rc::new(palette(shift)));
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
        usage: None,
    }
}

/// Result options of a recorded case.
pub fn options(options: &Value) -> ToolRenderResultOptions {
    ToolRenderResultOptions {
        expanded: options["expanded"].as_bool().unwrap(),
        is_partial: options["isPartial"].as_bool().unwrap(),
    }
}

/// The expand keys a recorded context binds.
pub fn expand_keys(context: &Value) -> Vec<String> {
    context["bindings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|key| key.as_str().unwrap().to_owned())
        .collect()
}

/// Render a recorded call or result and return its rows.
pub fn render_case(case: &Case) -> Vec<String> {
    let recorded = &case.input["context"];
    let setup = Setup::from(recorded);
    let _bindings = bind(&expand_keys(recorded));
    let env = Env::from_setup(&setup);
    let tool = create_read_tool_definition::<()>(
        "/work".into(),
        maestro_tools::ReadToolOptions::default(),
    );
    let args = &case.input["args"];
    let ctx = context(args, &setup, &env, None);
    let row = if case.op == "call" {
        (tool.render_call.as_ref().unwrap())(Some(args), &env.live, &ctx)
    } else {
        let result = read_result(&case.input["result"]);
        let options = options(&case.input["options"]);
        crate::display_support::native(
            tool.render_result.as_ref().unwrap(),
            &maestro_tools::output::render_utils::ToolRenderResultLike::from(&result),
            options,
            &env.live,
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
    Rc::new(maestro_theme::load_theme_from_path(&path, mode).unwrap())
}

/// Invoke a native callback with conversion only at its raw boundary.
pub fn native<D: serde::Serialize, S, E: maestro_tools::ToolExecution>(
    callback: &maestro_tools::ToolRenderResult<D, S, E>,
    result: &maestro_tools::output::render_utils::ToolRenderResultLike<'_, D>,
    options: ToolRenderResultOptions,
    theme: &LiveTheme,
    context: &ToolRenderContext<'_, S>,
) -> Result<ComponentHandle, maestro_models::DiagnosticErrorInfo> {
    callback.invoke_native((result, options), theme, context, |details| {
        serde_json::to_value(details)
            .map_err(|error| maestro_models::DiagnosticErrorInfo::error(error.to_string()))
    })
}
