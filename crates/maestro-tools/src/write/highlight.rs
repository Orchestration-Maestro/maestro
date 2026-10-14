//! Incrementally highlighted content of a streamed write call.

use crate::{
    output::render_utils::{normalize_display_text, replace_tabs},
    types::ToolRenderContext,
};
use maestro_theme::{get_language_from_path, highlight_code};

/// Rows re-highlighted together at the start of the content when a cached update extends it.
const FULL_HIGHLIGHT_LINES: usize = 50;

/// The rows of `code` in `language`.
pub(super) fn rows(context: &ToolRenderContext<'_>, code: &str, language: &str) -> Vec<String> {
    highlight_code(
        &context.environment.live_theme,
        &*context.environment.highlighter,
        code,
        Some(language),
    )
}

/// The first row of `line` in `language`.
fn row(context: &ToolRenderContext<'_>, line: &str, language: &str) -> String {
    rows(context, line, language)
        .into_iter()
        .next()
        .unwrap_or_default()
}

/// Highlighted rows of the content received so far.
pub(super) struct HighlightCache {
    /// Path the language came from.
    raw_path: String,
    /// Language of the path.
    language: &'static str,
    /// Content as received, kept to recognize an extension of it.
    raw_content: String,
    /// Content as displayed, split into rows.
    normalized_lines: Vec<String>,
    /// Highlighted counterpart of every row of `normalized_lines`.
    pub(super) highlighted_lines: Vec<String>,
}

/// The language of a nonempty path.
pub(super) fn language_of(raw_path: &str) -> Option<&'static str> {
    Some(raw_path)
        .filter(|path| !path.is_empty())
        .and_then(get_language_from_path)
}

/// The content as displayed: no carriage returns, tabs expanded.
fn displayed(text: &str) -> String {
    replace_tabs(&normalize_display_text(text))
}

/// Highlight all of `content`, or return nothing when the path names no language.
pub(super) fn rebuild(
    raw_path: &str,
    text: &str,
    context: &ToolRenderContext<'_>,
) -> Option<HighlightCache> {
    let language = language_of(raw_path)?;
    let normalized = displayed(text);
    Some(HighlightCache {
        raw_path: raw_path.to_owned(),
        language,
        raw_content: text.to_owned(),
        normalized_lines: normalized.split('\n').map(str::to_owned).collect(),
        highlighted_lines: rows(context, &normalized, language),
    })
}

/// Highlight the first rows together so multi-row constructs keep their colors.
fn refresh_prefix(cache: &mut HighlightCache, context: &ToolRenderContext<'_>) {
    let count = FULL_HIGHLIGHT_LINES.min(cache.normalized_lines.len());
    let source = cache.normalized_lines[..count].join("\n");
    let mut prefix = rows(context, &source, cache.language).into_iter();
    for (slot, line) in cache
        .highlighted_lines
        .iter_mut()
        .zip(&cache.normalized_lines)
        .take(count)
    {
        *slot = prefix
            .next()
            .unwrap_or_else(|| row(context, line, cache.language));
    }
}

/// Bring the cache up to date with `text`: when it extends the cached content of the same
/// path, re-highlight the changed last line, the new lines and the first rows; otherwise
/// highlight it all.
pub(super) fn update(
    cache: Option<HighlightCache>,
    raw_path: &str,
    text: &str,
    context: &ToolRenderContext<'_>,
) -> Option<HighlightCache> {
    let language = language_of(raw_path)?;
    let Some(mut cache) = cache.filter(|cache| {
        cache.language == language
            && cache.raw_path == raw_path
            && text.starts_with(&cache.raw_content)
    }) else {
        return rebuild(raw_path, text, context);
    };
    let Some(delta) = text
        .get(cache.raw_content.len()..)
        .filter(|delta| !delta.is_empty())
    else {
        return Some(cache);
    };
    let displayed_delta = displayed(delta);
    text.clone_into(&mut cache.raw_content);
    let mut segments = displayed_delta.split('\n');
    if let (Some(first), Some(last), Some(slot)) = (
        segments.next(),
        cache.normalized_lines.last_mut(),
        cache.highlighted_lines.last_mut(),
    ) {
        last.push_str(first);
        *slot = row(context, last, language);
    }
    for segment in segments {
        cache.normalized_lines.push(segment.to_owned());
        cache
            .highlighted_lines
            .push(row(context, segment, language));
    }
    refresh_prefix(&mut cache, context);
    Some(cache)
}
