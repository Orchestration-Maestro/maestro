//! The footer that shows the working directory, cumulative usage and the selected model.
use super::{shorten_path_with_home, thinking_selector::thinking_text};
use maestro_app::{
    AgentSession, DiagnosticErrorInfo, ExtensionExecution, Local, ReadonlyFooterDataProvider,
    ReadonlySessionManager, ThinkingLevel, is_whitespace_scalar,
};
use maestro_theme::{Collator, LiveTheme, ThemeColor, paint};
use maestro_tui::{Component, TruncateOptions, truncate_to_width, visible_width};
use serde_json::Value;
use std::{
    cell::{Cell, RefCell},
    error::Error,
    rc::Rc,
    sync::PoisonError,
};

/// The smallest gap between the stats and the model.
const MIN_PADDING: usize = 2;

/// Reads an environment variable, such as `HOME`.
pub type Environment = Rc<dyn Fn(&str) -> Option<String>>;

/// The context state a paint shows.
#[derive(Clone, Copy)]
struct Context {
    /// Usage as a percentage of the window, or `None` while tokens are unknown.
    percent: Option<f64>,
    /// The window the percentage is measured against.
    window: f64,
    /// Whether the session reports no usage at all, rather than unknown tokens.
    unstated: bool,
}

/// A stored record the footer cannot read.
fn record_error(field: &str) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo::new(
        Some("TypeError".to_owned()),
        format!("the stored {field} is missing or not a number"),
    )
}

/// The cumulative totals of every answer in the history.
#[derive(Clone, Copy)]
struct Totals {
    /// Reported input tokens.
    input: f64,
    /// Reported output tokens.
    output: f64,
    /// Reported cache reads.
    cache_read: f64,
    /// Reported cache writes.
    cache_write: f64,
    /// Reported cost.
    cost: f64,
}

impl Totals {
    /// The totals before any answer contributed.
    const EMPTY: Self = Self {
        input: 0.0,
        output: 0.0,
        cache_read: 0.0,
        cache_write: 0.0,
        cost: 0.0,
    };
}

/// The footer over a session, its read-only metadata, the published theme and the environment.
pub struct FooterComponent<E: ExtensionExecution = Local> {
    /// The session whose history and selections are rendered.
    session: RefCell<Rc<AgentSession<E>>>,
    /// The branch, provider count and extension statuses.
    footer_data: Rc<dyn ReadonlyFooterDataProvider>,
    /// The published theme, sampled at each paint.
    theme: LiveTheme,
    /// The stable status-key comparison.
    collator: Rc<Collator>,
    /// Reads `HOME` and `USERPROFILE` where the path is rendered.
    environment: Environment,
    /// Whether the context display carries the automatic-compaction marker.
    auto_compact_enabled: Cell<bool>,
}

impl<E: ExtensionExecution> FooterComponent<E> {
    /// Builds a footer over the supplied owners.
    pub fn new(
        session: Rc<AgentSession<E>>,
        footer_data: Rc<dyn ReadonlyFooterDataProvider>,
        theme: LiveTheme,
        collator: Rc<Collator>,
        environment: Environment,
    ) -> Self {
        Self {
            session: RefCell::new(session),
            footer_data,
            theme,
            collator,
            environment,
            auto_compact_enabled: Cell::new(true),
        }
    }

    /// Renders the supplied session from now on, releasing the previous one.
    pub fn set_session(&self, session: Rc<AgentSession<E>>) {
        let previous = self.session.replace(session);
        drop(previous);
    }

    /// Whether the context display carries the automatic-compaction marker.
    pub fn set_auto_compact_enabled(&self, enabled: bool) {
        self.auto_compact_enabled.set(enabled);
    }

    /// Releases nothing: the footer owns no work of its own.
    pub fn dispose(&self) {}

    /// The retained session, borrowed only for the clone.
    fn session(&self) -> Rc<AgentSession<E>> {
        Rc::clone(&self.session.borrow())
    }

    /// The working directory with its shortened home, its branch and the session name.
    fn path(&self, session: &AgentSession<E>) -> Result<String, Box<dyn Error>> {
        let cwd = session.session_manager().borrow().get_cwd().to_owned();
        let mut pwd = shorten_path_with_home(&cwd, &*self.environment);
        if let Some(branch) = self.footer_data.get_git_branch().filter(|b| !b.is_empty()) {
            pwd = format!("{pwd} ({branch})");
        }
        let manager = session.session_manager();
        let manager = manager.borrow();
        let name = manager
            .get_session_name()
            .map_err(Box::new)?
            .filter(|name| !name.is_empty());
        match name {
            Some(name) => Ok(format!("{pwd} • {name}")),
            None => Ok(pwd),
        }
    }

    /// The path row: dim, shortened with a dim ellipsis and never wider than the viewport.
    fn path_line(&self, width: usize, pwd: &str) -> Result<String, Box<dyn Error>> {
        let ellipsis = self.dim("...")?;
        let painted = self.dim(pwd)?;
        Ok(truncate_to_width(
            &painted,
            width,
            TruncateOptions {
                ellipsis: &ellipsis,
                pad: false,
            },
        ))
    }

    /// Dresses text in the role the published theme supplies now.
    fn painted(&self, color: &ThemeColor, text: &str) -> Result<String, Box<dyn Error>> {
        let theme = self.theme.get().map_err(Box::new)?;
        Ok(paint(Some(&theme), color, text))
    }

    /// Dresses text in the dim role.
    fn dim(&self, text: &str) -> Result<String, Box<dyn Error>> {
        self.painted(&ThemeColor::Dim, text)
    }

    /// The right side without the provider: the model identifier and its thinking level.
    fn model_side(session: &AgentSession<E>) -> String {
        let (id, reasoning) = {
            let model = session.model();
            let descriptor = model.read().unwrap_or_else(PoisonError::into_inner);
            (descriptor.id.clone(), descriptor.reasoning)
        };
        let name = if id.is_empty() {
            "no-model".to_owned()
        } else {
            id
        };
        if !reasoning {
            return name;
        }
        match session.thinking_level() {
            ThinkingLevel::Off => format!("{name} • thinking off"),
            level => format!("{name} • {}", thinking_text(&level).0),
        }
    }

    /// The right side with the provider prefix while several providers exist and it fits.
    fn provider_side(
        &self,
        session: &AgentSession<E>,
        right: String,
        left: usize,
        width: usize,
    ) -> String {
        if self.footer_data.get_available_provider_count() <= 1.0 {
            return right;
        }
        let model = session.model();
        let prefixed = {
            let descriptor = model.read().unwrap_or_else(PoisonError::into_inner);
            format!("({}) {right}", descriptor.provider)
        };
        if left + MIN_PADDING + visible_width(&prefixed) > width {
            right
        } else {
            prefixed
        }
    }

    /// The stats row: usage and context on the left, the model on the right, dim wrapped.
    fn stats(
        &self,
        session: &AgentSession<E>,
        totals: Totals,
        context: Context,
        width: usize,
    ) -> Result<String, Box<dyn Error>> {
        let mut parts = Vec::new();
        for (marker, total) in [
            ("↑", totals.input),
            ("↓", totals.output),
            ("R", totals.cache_read),
            ("W", totals.cache_write),
        ] {
            if displayed(total) {
                parts.push(format!("{marker}{}", format_tokens(total)));
            }
        }
        if let Some(cost) = cost_part(session, totals.cost) {
            parts.push(cost);
        }
        parts.push(self.context_part(context)?);
        let mut left = parts.join(" ");
        let mut left_width = visible_width(&left);
        if left_width > width {
            left = truncate_to_width(
                &left,
                width,
                TruncateOptions {
                    ellipsis: "...",
                    pad: false,
                },
            );
            left_width = visible_width(&left);
        }
        let right = self.provider_side(session, Self::model_side(session), left_width, width);
        let remainder = layout(width, left_width, &right);
        Ok(format!("{}{}", self.dim(&left)?, self.dim(&remainder)?))
    }

    /// The context display, colored by its reached threshold role.
    fn context_part(&self, context: Context) -> Result<String, Box<dyn Error>> {
        let value = context.percent.unwrap_or(0.0);
        let marker = if self.auto_compact_enabled.get() {
            " (auto)"
        } else {
            ""
        };
        let shown = format_tokens(context.window);
        let display = if context.percent.is_none() && !context.unstated {
            format!("?/{shown}{marker}")
        } else {
            format!("{}%/{shown}{marker}", fixed(value, 1))
        };
        if value > 90.0 {
            return self.painted(&ThemeColor::Error, &display);
        }
        if value > 70.0 {
            return self.painted(&ThemeColor::Warning, &display);
        }
        Ok(display)
    }

    /// Appends the status row when the metadata holds any status.
    fn statuses(
        &self,
        width: usize,
        mut lines: Vec<String>,
    ) -> Result<Vec<String>, Box<dyn Error>> {
        let statuses = self.footer_data.get_extension_statuses();
        if statuses.is_empty() {
            return Ok(lines);
        }
        let mut pairs: Vec<(String, String)> = statuses.iter().collect();
        pairs.sort_by(|left, right| self.collator.compare(&left.0, &right.0));
        let text = pairs
            .iter()
            .map(|(_, value)| sanitize_status_text(value))
            .collect::<Vec<_>>()
            .join(" ");
        let ellipsis = self.dim("...")?;
        lines.push(truncate_to_width(
            &text,
            width,
            TruncateOptions {
                ellipsis: &ellipsis,
                pad: false,
            },
        ));
        Ok(lines)
    }
}

impl<E: ExtensionExecution + 'static> Component for FooterComponent<E> {
    fn render(&self, width: usize) -> Vec<String> {
        self.try_render(width).unwrap_or_default()
    }

    fn try_render(&self, width: usize) -> Result<Vec<String>, Box<dyn Error>> {
        let session = self.session();
        let totals = totals(&session)?;
        let usage = session.get_context_usage().map_err(Box::new)?;
        let context = Context {
            percent: usage.as_ref().and_then(|usage| usage.percent),
            window: usage
                .as_ref()
                .map_or_else(|| model_window(&session), |usage| usage.context_window),
            unstated: usage.is_none(),
        };
        let pwd = self.path(&session)?;
        let stats = self.stats(&session, totals, context, width)?;
        let lines = vec![self.path_line(width, &pwd)?, stats];
        self.statuses(width, lines)
    }

    fn invalidate(&self) {}
}

/// The cumulative totals of every answer in the retained history.
fn totals<E: ExtensionExecution>(session: &AgentSession<E>) -> Result<Totals, Box<dyn Error>> {
    let manager = session.session_manager().borrow();
    let mut totals = Totals::EMPTY;
    for entry in manager.get_entries() {
        let message = entry.get("message").unwrap_or(&Value::Null);
        if entry.get("type").and_then(Value::as_str) != Some("message")
            || message.get("role").and_then(Value::as_str) != Some("assistant")
        {
            continue;
        }
        let usage = message
            .get("usage")
            .filter(|usage| usage.is_object())
            .ok_or_else(|| record_error("usage"))?;
        totals.input += reported(usage, "input");
        totals.output += reported(usage, "output");
        totals.cache_read += reported(usage, "cacheRead");
        totals.cache_write += reported(usage, "cacheWrite");
        let cost = usage
            .get("cost")
            .filter(|cost| cost.is_object())
            .ok_or_else(|| record_error("usage.cost"))?;
        totals.cost += reported(cost, "total");
    }
    Ok(totals)
}

/// The context window of the selected model.
fn model_window<E: ExtensionExecution>(session: &AgentSession<E>) -> f64 {
    let model = session.model();
    model
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .context_window
}

/// The cost part, shown while a cost was reported or the model is on a subscription.
fn cost_part<E: ExtensionExecution>(session: &AgentSession<E>, cost: f64) -> Option<String> {
    let model = session.model();
    let subscription = session.model_registry().is_using_oauth(&model);
    if !displayed(cost) && !subscription {
        return None;
    }
    let marker = if subscription { " (sub)" } else { "" };
    Some(format!("${}{marker}", fixed(cost, 3)))
}

/// The stats row after its left text: the padding and the room left for the right side.
fn layout(width: usize, left_width: usize, right: &str) -> String {
    let right_width = visible_width(right);
    if left_width + MIN_PADDING + right_width <= width {
        let padding = " ".repeat(width - left_width - right_width);
        return format!("{padding}{right}");
    }
    let Some(available) = width
        .checked_sub(left_width + MIN_PADDING)
        .filter(|available| *available > 0)
    else {
        return String::new();
    };
    let truncated = truncate_to_width(
        right,
        available,
        TruncateOptions {
            ellipsis: "",
            pad: false,
        },
    );
    let truncated_width = visible_width(&truncated);
    let padding = " ".repeat(width.saturating_sub(left_width + truncated_width));
    format!("{padding}{truncated}")
}

/// A stored numeric field: absent or non-numeric poisons the total, null contributes zero.
fn reported(record: &Value, field: &str) -> f64 {
    match record.get(field) {
        Some(Value::Number(number)) => number.as_f64().unwrap_or(f64::NAN),
        Some(Value::Null) => 0.0,
        Some(_) | None => f64::NAN,
    }
}

/// Whether a total is shown, which an unread total never is.
fn displayed(value: f64) -> bool {
    !value.is_nan() && value != 0.0
}

/// The token spelling: plain, tenths of `k`, whole `k`, tenths of `M`, whole `M`.
fn format_tokens(count: f64) -> String {
    let mut buffer = ryu_js::Buffer::new();
    if count < 1000.0 {
        return buffer.format(count).to_owned();
    }
    if count < 10000.0 {
        return format!("{}k", buffer.format_to_fixed(count / 1000.0, 1));
    }
    if count < 1_000_000.0 {
        return format!("{}k", buffer.format(half_up(count / 1000.0)));
    }
    if count < 10_000_000.0 {
        return format!("{}M", buffer.format_to_fixed(count / 1_000_000.0, 1));
    }
    format!("{}M", buffer.format(half_up(count / 1_000_000.0)))
}

/// A fixed-point spelling with exactly `digits` fraction digits.
fn fixed(value: f64, digits: u8) -> String {
    let mut buffer = ryu_js::Buffer::new();
    buffer.format_to_fixed(value, digits).to_owned()
}

/// The nearest whole number, with halves rounded towards the larger value.
fn half_up(value: f64) -> f64 {
    (value + 0.5).floor()
}

/// Replaces tabs and line breaks with spaces, collapses space runs and trims outer whitespace.
fn sanitize_status_text(text: &str) -> String {
    let mut collapsed = String::with_capacity(text.len());
    for character in text.chars() {
        let replaced = match character {
            '\r' | '\n' | '\t' => ' ',
            other => other,
        };
        if replaced == ' ' && collapsed.ends_with(' ') {
            continue;
        }
        collapsed.push(replaced);
    }
    collapsed.trim_matches(is_whitespace_scalar).to_owned()
}
