//! Inline formatting and parent-prefix restoration.
use super::{
    Renderer, TextDecoration,
    parse::{Kind, Node},
};
use std::borrow::Cow;

/// The parent formatting applied to plain text.
#[derive(Clone, Copy)]
pub(super) enum Style<'a> {
    /// Message base style.
    Default,
    /// Quote children retain no message foreground.
    Quote(&'a str),
    /// Heading decoration by level.
    Heading(usize),
}
impl Renderer<'_> {
    /// Applies one parent style to text.
    pub(super) fn style(&self, text: &str, style: Style<'_>) -> String {
        match style {
            Style::Default => self.default_style(text),
            Style::Quote(_) => text.to_owned(),
            Style::Heading(level) => {
                let text = if level == 1 {
                    (self.theme.underline)(text)
                } else {
                    text.to_owned()
                };
                (self.theme.heading)(&(self.theme.bold)(&text))
            }
        }
    }
    /// Renders inline children, restoring parent styling after inline code.
    pub(super) fn inline<'a>(
        &self,
        nodes: impl IntoIterator<Item = &'a Node>,
        style: Style<'_>,
    ) -> String {
        let prefix = self.prefix(style);
        self.inline_with_prefix(nodes, style, &prefix)
    }
    /// Reuses the parent's prefix through nested formatted spans.
    fn inline_with_prefix<'a>(
        &self,
        nodes: impl IntoIterator<Item = &'a Node>,
        style: Style<'_>,
        prefix: &str,
    ) -> String {
        let mut result = String::new();
        for node in nodes {
            let (text, restore) = match &node.kind {
                Kind::Text(text) => (
                    text.decoded
                        .split('\n')
                        .map(|part| self.style(part, style))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    false,
                ),
                Kind::Html(text)
                | Kind::HtmlBlock(text)
                | Kind::Image(text)
                | Kind::Heading(_, _, text)
                | Kind::Quote(_, text) => (
                    text.split('\n')
                        .map(|part| self.style(part, style))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    false,
                ),
                Kind::Code(text) => ((self.theme.code)(text), true),
                Kind::Emphasis(children) => (
                    (self.theme.italic)(&self.inline_with_prefix(children, style, prefix)),
                    true,
                ),
                Kind::Strong(children) => (
                    (self.theme.bold)(&self.inline_with_prefix(children, style, prefix)),
                    true,
                ),
                Kind::Strike(children) => (
                    (self.theme.strikethrough)(&self.inline_with_prefix(children, style, prefix)),
                    true,
                ),
                Kind::Link(children, label, href) => (
                    self.link(
                        &self.inline_with_prefix(children, style, prefix),
                        label,
                        href,
                    ),
                    true,
                ),
                Kind::Break => ("\n".to_owned(), false),
                _ => (String::new(), false),
            };
            result.push_str(&text);
            if restore {
                result.push_str(prefix);
            }
        }
        if !prefix.is_empty() {
            while result.ends_with(prefix) {
                result.truncate(result.len() - prefix.len());
            }
        }
        result
    }
    /// Selects clickable transport or the printed destination suffix.
    fn link(&self, content: &str, label: &str, href: &str) -> String {
        let styled = (self.theme.link)(&(self.theme.underline)(content));
        if self.terminal_image.get_capabilities().hyperlinks {
            crate::hyperlink(&styled, href)
        } else if label == href || label == href.strip_prefix("mailto:").unwrap_or(href) {
            styled
        } else {
            format!("{styled}{}", (self.theme.link_url)(&format!(" ({href})")))
        }
    }
    /// Extracts the sentinel prefix, or nothing when the callback removes it.
    pub(super) fn prefix<'a>(&'a self, style: Style<'a>) -> Cow<'a, str> {
        match style {
            Style::Default => Cow::Borrowed(
                self.default_prefix
                    .get_or_init(|| self.extract_prefix(style)),
            ),
            Style::Quote(prefix) => Cow::Borrowed(prefix),
            Style::Heading(_) => Cow::Owned(self.extract_prefix(style)),
        }
    }
    /// Extracts one parent prefix without storing it.
    pub(super) fn extract_prefix(&self, style: Style<'_>) -> String {
        let styled = if matches!(style, Style::Quote(_)) {
            (self.theme.quote)(&(self.theme.italic)("\0"))
        } else {
            self.style("\0", style)
        };
        styled
            .split_once('\0')
            .map_or_else(String::new, |(prefix, _)| prefix.to_owned())
    }
    /// Applies foreground and distinct decorations in their fixed order.
    fn default_style(&self, text: &str) -> String {
        let Some(base) = self.default_style else {
            return text.to_owned();
        };
        let mut result = base
            .color
            .as_ref()
            .map_or_else(|| text.to_owned(), |color| color(text));
        for (enabled, apply) in [
            (TextDecoration::Bold, &self.theme.bold),
            (TextDecoration::Italic, &self.theme.italic),
            (TextDecoration::Strikethrough, &self.theme.strikethrough),
            (TextDecoration::Underline, &self.theme.underline),
        ] {
            if base.decorations.contains(&enabled) {
                result = apply(&result);
            }
        }
        result
    }
}
