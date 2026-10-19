//! Pure preparation of plain or explicitly highlighted expandable output.
use maestro_app::markup::{EscapeMode, escape};
use maestro_theme::SyntaxHighlighter;
use std::fmt::Write as _;

/// Prepare output with separate full and preview bodies and an exact remaining-line hint.
/// Empty language labels select plain rows. Highlight failures fall back per body.
#[must_use]
pub fn format_expandable_output(
    text: &str,
    max_lines: usize,
    lang: Option<&str>,
    highlighter: &dyn SyntaxHighlighter,
) -> String {
    let text = maestro_app::replace_tabs(text);
    let lines: Vec<_> = text.split('\n').collect();
    let preview = &lines[..max_lines.min(lines.len())];
    let lang = lang.filter(|lang| !lang.is_empty());
    let full = body(&text, &lines, lang, highlighter);
    let remaining = lines.len().saturating_sub(max_lines);
    if remaining == 0 {
        return format!("<div class=\"tool-output\">{full}</div>");
    }
    let preview = body(&preview.join("\n"), preview, lang, highlighter);
    format!(
        "<div class=\"tool-output expandable\"><div class=\"output-preview\">{preview}<div class=\"expand-hint\">... ({remaining} more lines)</div></div><div class=\"output-full\">{full}</div></div>"
    )
}

/// Render a complete code operand or preserve each plain row, including empty rows.
fn body(
    text: &str,
    lines: &[&str],
    lang: Option<&str>,
    highlighter: &dyn SyntaxHighlighter,
) -> String {
    if let Some(lang) = lang {
        let html = maestro_theme::highlight_html(highlighter, text, Some(lang))
            .ok()
            .flatten()
            .unwrap_or_else(|| escape(text, EscapeMode::HtmlDecimal));
        return format!("<pre><code class=\"hljs\">{html}</code></pre>");
    }
    let mut html = String::new();
    for line in lines {
        let _ = write!(html, "<div>{}</div>", escape(line, EscapeMode::HtmlDecimal));
    }
    html
}

/// Install scoped viewer styles and root-owned output disclosure listeners.
/// Retain the returned owner in the mounted parent's `ReadingRoot::owner`.
/// # Errors
/// Returns document, stylesheet, presentation or listener-installation failures.
/// Later selection or class-toggle failures are thrown by the native click callback.
#[cfg(target_arch = "wasm32")]
pub fn bind_expandable_output(
    root: &web_sys::Element,
) -> Result<Option<Box<dyn std::any::Any>>, crate::browser::attachment_content::AttachmentHostError>
{
    use crate::browser::{
        attachment_content::browser::host_error,
        disclosure::{Disclosure, bind},
    };
    let document = root.owner_document().ok_or_else(|| {
        crate::browser::attachment_content::AttachmentHostError::Host(
            "missing output document".into(),
        )
    })?;
    crate::browser::install_stylesheet(
        &document,
        "maestro-export-output",
        include_str!("output.css"),
    )?;
    crate::browser::install_stylesheet(
        &document,
        "maestro-export-markdown",
        include_str!("markdown.css"),
    )?;
    let syntax = maestro_theme::highlight_css().map_err(|error| {
        crate::browser::attachment_content::AttachmentHostError::Presentation(error.to_string())
    })?;
    crate::browser::install_stylesheet(&document, "maestro-export-syntax", &syntax)?;
    let mut state = Vec::new();
    if root
        .matches(".tool-output.expandable")
        .map_err(|e| host_error(&e))?
    {
        state.push(Disclosure {
            element: root.clone(),
            class: "expanded",
        });
    }
    let nodes = root
        .query_selector_all(".tool-output.expandable")
        .map_err(|e| host_error(&e))?;
    for index in 0..nodes.length() {
        if let Some(element) = nodes
            .item(index)
            .map(wasm_bindgen::JsCast::unchecked_into::<web_sys::Element>)
        {
            state.push(Disclosure {
                element,
                class: "expanded",
            });
        }
    }
    bind(state)
}
