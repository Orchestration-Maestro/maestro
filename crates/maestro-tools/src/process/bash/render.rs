//! Retained shell call and result rendering.
use crate::{
    ToolRenderContext, ToolRenderResultOptions,
    output::{
        render_parts::{downcast, expand_hint, into_text, num, paint, plain_expand_hint, themed},
        render_utils::{
            ToolRenderResultLike, TruncationView, get_text_output, invalid_arg_text, number_text,
            render_args, str,
        },
        visual_truncate::{VisualTruncateResult, truncate_to_visual_lines},
    },
};
use maestro_models::DiagnosticErrorInfo;
use maestro_theme::{LiveTheme, Theme, ThemeColor};
use maestro_tui::tui::{ComponentHandle, RepeatingTimer};
use maestro_tui::{Component, Container, Text, truncate_to_width};
use num_traits::ToPrimitive;
use serde_json::Value;
use std::{cell::RefCell, rc::Rc, time::Duration};

/// Timing shared by a shell row's call and result slots.
#[derive(Default)]
pub struct BashRenderState {
    /// First execution timestamp in whole runtime milliseconds.
    started_at: Option<u128>,
    /// First settlement timestamp in whole runtime milliseconds.
    ended_at: Option<u128>,
    /// Owned refresh continuation, cancelled when the last row owner disappears.
    timer: Option<RepeatingTimer>,
}
/// Draw authored arguments with the published theme.
pub(super) fn render_call(
    args: Option<&Value>,
    _theme: &LiveTheme,
    context: &ToolRenderContext<'_, BashRenderState>,
) -> Result<ComponentHandle, DiagnosticErrorInfo> {
    let args = render_args(args);
    if context.phase.execution_started {
        let now = context.environment.runtime.now().as_millis();
        let mut state = context.state.borrow_mut();
        if state.started_at.is_none() {
            state.started_at = Some(now);
            state.ended_at = None;
        }
    }
    let theme = themed(context.environment.live_theme.get())?;
    let command = match str(args.get("command")) {
        None => themed(invalid_arg_text(&theme))?,
        Some("") => paint(&theme, &ThemeColor::ToolOutput, "...")?,
        Some(command) => command.to_owned(),
    };
    let mut title = paint(
        &theme,
        &ThemeColor::ToolTitle,
        &Theme::bold(&format!("$ {command}"), true),
    )?;
    if let Some(timeout) = args
        .get("timeout")
        .and_then(Value::as_f64)
        .filter(|timeout| *timeout != 0.0)
    {
        title.push_str(&paint(
            &theme,
            &ThemeColor::Muted,
            &format!(" (timeout {}s)", num(timeout)),
        )?);
    }
    Ok(into_text(context.last_component.as_ref(), title))
}

/// Settle the clock or admit one refresh continuation without retaining the row.
fn timing(
    options: ToolRenderResultOptions,
    context: &ToolRenderContext<'_, BashRenderState>,
) -> Option<u128> {
    let now = context.environment.runtime.now().as_millis();
    let (start, end, needs_timer) = {
        let mut state = context.state.borrow_mut();
        if !options.is_partial || context.is_error {
            state.ended_at.get_or_insert(now);
        }
        (
            state.started_at,
            state.ended_at,
            state.started_at.is_some()
                && options.is_partial
                && !context.is_error
                && state.timer.is_none(),
        )
    };
    if needs_timer {
        let weak = Rc::downgrade(&context.state);
        let invalidate = Rc::clone(&context.invalidate);
        let timer = RepeatingTimer::new(
            Rc::clone(&context.environment.runtime),
            Duration::from_secs(1),
            Rc::new(move || {
                if let Some(_owner) = weak.upgrade() {
                    invalidate();
                }
            }),
        );
        let replaced = context.state.borrow_mut().timer.replace(timer);
        drop(replaced);
    }
    if !options.is_partial || context.is_error {
        let timer = context.state.borrow_mut().timer.take();
        drop(timer);
    }
    start.map(|start| end.unwrap_or(now).saturating_sub(start))
}
/// Draw the first captured duration through the existing text component.
pub(super) fn render_result(
    result: &ToolRenderResultLike<'_, Value>,
    options: ToolRenderResultOptions,
    _theme: &LiveTheme,
    context: &ToolRenderContext<'_, BashRenderState>,
) -> Result<ComponentHandle, DiagnosticErrorInfo> {
    let elapsed = timing(options, context);
    let theme = themed(context.environment.live_theme.get())?;
    let component = context
        .last_component
        .as_ref()
        .and_then(downcast::<BashResultRenderComponent>)
        .unwrap_or_else(|| Rc::new(BashResultRenderComponent::default()));
    component.frame.clear();
    let output = get_text_output(
        Some(result),
        context.show_images,
        &context.environment.terminal_image,
    )?;
    let mut output = output.trim_matches(maestro_text::is_whitespace_scalar);
    if !options.is_partial {
        output = without_footer(output, result.details);
    }
    if !output.is_empty() {
        let styled = output
            .split('\n')
            .map(|line| paint(&theme, &ThemeColor::ToolOutput, line))
            .collect::<Result<Vec<_>, _>>()?
            .join("\n");
        if options.expanded {
            component
                .frame
                .add_child(Rc::new(Text::new(format!("\n{styled}"), 0, 0, None)));
        } else {
            paint(&theme, &ThemeColor::Muted, "")?;
            paint(&theme, &ThemeColor::Dim, "")?;
            component.frame.add_child(Rc::new(VisualTail {
                text: styled,
                live_theme: context.environment.live_theme.clone(),
                cache: RefCell::default(),
            }));
        }
    }
    if let Some(notice) = metadata(result.details) {
        let notice = paint(&theme, &ThemeColor::Warning, &notice)?;
        component
            .frame
            .add_child(Rc::new(Text::new(format!("\n{notice}"), 0, 0, None)));
    }
    if let Some(elapsed) = elapsed {
        let label = if options.is_partial {
            "Elapsed"
        } else {
            "Took"
        };
        let mut buffer = ryu_js::Buffer::new();
        let seconds = buffer.format_to_fixed(elapsed.to_f64().unwrap_or(f64::MAX) / 1000.0, 1);
        let text = paint(&theme, &ThemeColor::Muted, &format!("{label} {seconds}s"))?;
        component
            .frame
            .add_child(Rc::new(Text::new(format!("\n{text}"), 0, 0, None)));
    }
    Ok(component)
}

/// Result slot identity with layout owned by the toolkit container.
#[derive(Default)]
struct BashResultRenderComponent {
    /// Rebuilt body, notices and elapsed text.
    frame: Container,
}
impl Component for BashResultRenderComponent {
    fn render(&self, width: usize) -> Vec<String> {
        self.frame.render(width)
    }
    fn invalidate(&self) {
        self.frame.invalidate();
    }
}
/// Width-sensitive tail with a live hint outside the cached body.
struct VisualTail {
    /// Styled body captured by the result callback.
    text: String,
    /// Current hint role colors.
    live_theme: LiveTheme,
    /// Wrapping result for the last viewport.
    cache: RefCell<Option<(usize, VisualTruncateResult)>>,
}
impl Component for VisualTail {
    fn render(&self, width: usize) -> Vec<String> {
        let (skipped, lines) = {
            let mut cache = self.cache.borrow_mut();
            if cache.as_ref().is_none_or(|(old, _)| *old != width) {
                *cache = Some((width, truncate_to_visual_lines(&self.text, 5, width, None)));
            }
            cache
                .as_ref()
                .map(|(_, tail)| (tail.skipped_count, tail.visual_lines.clone()))
                .unwrap_or_default()
        };
        let mut rows = vec![String::new()];
        if skipped > 0 {
            let counts = format!("{skipped} earlier lines");
            let hint = self
                .live_theme
                .get()
                .ok()
                .and_then(|theme| expand_hint(&counts, &theme, &self.live_theme).ok())
                .unwrap_or_else(|| plain_expand_hint(&counts));
            rows.push(truncate_to_width(
                &hint.replacen('\n', "", 1),
                width,
                maestro_tui::TruncateOptions::default(),
            ));
        }
        rows.extend(lines);
        rows
    }
    fn invalidate(&self) {
        self.cache.take();
    }
}

/// `output` without the truncation footer the tool appended, when the details report a
/// truncation and a full-output path that the footer names; the notice row repeats it.
///
/// The footer starts at the last blank line before `[`, or at the very start when the kept
/// tail held only whitespace.
fn without_footer<'a>(output: &'a str, details: Option<&Value>) -> &'a str {
    let Some(details) = details else {
        return output;
    };
    let truncated = TruncationView::selected(Some(details)).is_some();
    let path = details
        .get("fullOutputPath")
        .and_then(Value::as_str)
        .filter(|path| !path.is_empty());
    let Some(path) = path.filter(|_| truncated && output.ends_with(']')) else {
        return output;
    };
    let start = output
        .rfind("\n\n[")
        .or_else(|| output.starts_with('[').then_some(0));
    match start {
        Some(start) if output[start..].contains(path) => {
            output[..start].trim_end_matches(maestro_text::is_whitespace_scalar)
        }
        _ => output,
    }
}

/// Artifact identity precedes active truncation, independently of body presence.
fn metadata(details: Option<&Value>) -> Option<String> {
    use crate::output::truncate::format_size;
    let details = details?;
    let mut warnings = Vec::new();
    if let Some(path) = details
        .get("fullOutputPath")
        .and_then(Value::as_str)
        .filter(|path| !path.is_empty())
    {
        warnings.push(format!("Full output: {path}"));
    }
    if let Some(truncation) = TruncationView::selected(Some(details)) {
        warnings.push(if truncation.by_lines() {
            format!(
                "Truncated: showing {} of {} lines",
                number_text(Some(truncation.0), "outputLines"),
                number_text(Some(truncation.0), "totalLines")
            )
        } else {
            format!(
                "Truncated: {} lines shown ({} limit)",
                number_text(Some(truncation.0), "outputLines"),
                format_size(truncation.max_bytes())
            )
        });
    }
    (!warnings.is_empty()).then(|| format!("[{}]", warnings.join(". ")))
}
