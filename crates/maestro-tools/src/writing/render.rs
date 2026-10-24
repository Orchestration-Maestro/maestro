//! Terminal rendering of write calls and results.
use crate::output::render_utils::{ToolRenderResultLike, render_args};

use super::highlight::{HighlightCache, language_of, rebuild, rows, update};
use crate::{
    output::{
        render_parts::{downcast, expand_hint, into_text, paint, themed},
        render_utils::{
            PREVIEW_LINES, normalize_display_text, path_display, replace_tabs, selected_path, str,
            without_trailing_empty,
        },
    },
    types::{ToolRenderContext, ToolRenderResultOptions},
};
use maestro_models::{DiagnosticErrorInfo, UserBlock};
use maestro_theme::{LiveTheme, Theme, ThemeColor};
use maestro_tui::{Component, Container, Text, interface::ComponentHandle};
use serde_json::Value;
use std::{borrow::Cow, cell::RefCell, rc::Rc};

/// Result of a render step.
type Rendered<T> = Result<T, DiagnosticErrorInfo>;

/// A write call row: its text and the highlighting of the content received so far.
struct WriteCall {
    /// The drawn call.
    text: Text,
    /// Highlighted rows, kept between redraws while the arguments stream in.
    cache: RefCell<Option<HighlightCache>>,
}

impl Component for WriteCall {
    fn render(&self, width: usize) -> Vec<String> {
        self.text.render(width)
    }

    fn invalidate(&self) {
        self.text.invalidate();
    }
}

/// The row's previous component when it is a write call, otherwise a new one.
fn call_component(previous: Option<&ComponentHandle>) -> Rc<WriteCall> {
    previous.and_then(downcast).unwrap_or_else(|| {
        Rc::new(WriteCall {
            text: Text::new(String::new(), 0, 0, None),
            cache: RefCell::new(None),
        })
    })
}

/// The rows of the content preview, highlighted when the path names a language.
fn preview_rows<'a>(
    body: &str,
    raw_path: &str,
    cache: Option<&'a HighlightCache>,
    context: &ToolRenderContext<'_>,
) -> Rendered<Cow<'a, [String]>> {
    Ok(match (language_of(raw_path), cache) {
        (Some(_), Some(cache)) => Cow::Borrowed(&cache.highlighted_lines),
        (Some(language), None) => Cow::Owned(rows(
            context,
            &replace_tabs(&normalize_display_text(body)),
            language,
        )?),
        (None, _) => Cow::Owned(
            normalize_display_text(body)
                .split('\n')
                .map(str::to_owned)
                .collect(),
        ),
    })
}

/// The content preview: the first rows, then how many more there are and how to expand.
fn format_preview(
    body: &str,
    raw_path: &str,
    cache: Option<&HighlightCache>,
    (theme, context): (&Theme, &ToolRenderContext<'_>),
) -> Rendered<String> {
    let highlighted = language_of(raw_path).is_some();
    let rendered = preview_rows(body, raw_path, cache, context)?;
    let lines = without_trailing_empty(&rendered);
    let limit = if context.expanded {
        lines.len()
    } else {
        PREVIEW_LINES
    };
    let shown = &lines[..limit.min(lines.len())];
    let rows = shown
        .iter()
        .map(|line| {
            if highlighted {
                Ok(Cow::Borrowed(line.as_str()))
            } else {
                paint(theme, &ThemeColor::ToolOutput, &replace_tabs(line)).map(Cow::Owned)
            }
        })
        .collect::<Rendered<Vec<_>>>()?;
    let mut text = format!("\n\n{}", rows.join("\n"));
    if lines.len() > shown.len() {
        let counts = format!(
            "{} more lines, {} total",
            lines.len() - shown.len(),
            lines.len()
        );
        text += &expand_hint(&counts, theme, &context.environment.live_theme)?;
    }
    Ok(text)
}

/// The call text: the title, then the content preview or why there is none.
fn format_call(
    args: &Value,
    cache: Option<&HighlightCache>,
    (theme, context): (&Theme, &ToolRenderContext<'_>),
) -> Rendered<String> {
    let display = path_display(selected_path(args), theme)?;
    let title = paint(theme, &ThemeColor::ToolTitle, &Theme::bold("write", true))?;
    let mut text = format!("{title} {display}");
    match str(args.get("content")) {
        None => {
            let notice = paint(
                theme,
                &ThemeColor::Error,
                "[invalid content arg - expected string]",
            )?;
            text.extend(["\n\n", &notice]);
        }
        Some("") => {}
        Some(body) => {
            let raw_path = str(selected_path(args)).unwrap_or_default();
            text += &format_preview(body, raw_path, cache, (theme, context))?;
        }
    }
    Ok(text)
}

/// Draw a write call, reusing the row's previous component and its highlighted rows.
///
/// # Errors
/// Returns a theme or home failure; the highlighting stays updated.
pub(super) fn render_call(
    args: Option<&Value>,
    theme: &LiveTheme,
    context: &ToolRenderContext<'_>,
) -> Rendered<ComponentHandle> {
    let args = render_args(args);
    let component = call_component(context.last_component.as_ref());
    let raw_path = str(selected_path(args)).unwrap_or_default();
    let previous = component.cache.take();
    let cache = match str(args.get("content")) {
        None => Ok(None),
        Some(body) if context.phase.args_complete => rebuild(raw_path, body, context),
        Some(body) => update(previous, raw_path, body, context),
    }?;
    component.cache.replace(cache);
    let resolved = themed(theme.get())?;
    let text = format_call(
        args,
        component.cache.borrow().as_ref(),
        (&resolved, context),
    )?;
    component.text.set_text(text);
    Ok(component)
}

/// Draw a write result: nothing for a success or an error without text, otherwise the error.
///
/// # Errors
/// Returns a theme failure; the previous component is left unchanged.
pub(super) fn render_result(
    result: &ToolRenderResultLike<'_, Value>,
    _options: ToolRenderResultOptions,
    theme: &LiveTheme,
    context: &ToolRenderContext<'_>,
) -> Rendered<ComponentHandle> {
    let output = UserBlock::text_output(result.content);
    if context.is_error && !output.is_empty() {
        let resolved = themed(theme.get())?;
        let error = paint(&resolved, &ThemeColor::Error, &output)?;
        return Ok(into_text(
            context.last_component.as_ref(),
            format!("\n{error}"),
        ));
    }
    let container = context
        .last_component
        .as_ref()
        .and_then(downcast::<Container>)
        .unwrap_or_else(|| Rc::new(Container::new()));
    container.clear();
    Ok(container)
}
