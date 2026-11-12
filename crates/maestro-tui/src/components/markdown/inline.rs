//! Inline formatting and parent-prefix restoration.
use super::{
    Renderer, TextDecoration,
    nodes::{Kind, Node},
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
/// The styling a formatted span applies once its children are rendered.
enum Wrap<'a> {
    /// Italic emphasis.
    Italic,
    /// Bold emphasis.
    Bold,
    /// Struck-through text.
    Strikethrough,
    /// Link label with its authored spelling and destination.
    Link {
        /// Authored label spelling.
        label: &'a str,
        /// Destination.
        href: &'a str,
    },
}

/// One run of sibling nodes being rendered, with the styling of the span that opened it.
struct Run<'a> {
    /// Siblings still to render.
    nodes: std::vec::IntoIter<&'a Node>,
    /// Text rendered so far.
    result: String,
    /// Styling the opening span applies to this run, absent for the outermost run.
    wrap: Option<Wrap<'a>>,
}
impl<'a> Run<'a> {
    /// Start a run over `nodes`.
    fn new(nodes: Vec<&'a Node>, wrap: Option<Wrap<'a>>) -> Self {
        Self {
            nodes: nodes.into_iter(),
            result: String::new(),
            wrap,
        }
    }
}

/// The children and styling of a formatted span, or nothing for any other node.
fn span(node: &Node) -> Option<(&[Node], Wrap<'_>)> {
    match &node.kind {
        Kind::Emphasis(children) => Some((children, Wrap::Italic)),
        Kind::Strong(children) => Some((children, Wrap::Bold)),
        Kind::Strike(children) => Some((children, Wrap::Strikethrough)),
        Kind::Link(children, label, href, _) => Some((children, Wrap::Link { label, href })),
        _ => None,
    }
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
    /// Clean native text before applying the parent style to each line.
    fn plain_inline(&self, text: &str, style: Style<'_>) -> String {
        self.clean(text)
            .split('\n')
            .map(|part| self.style(part, style))
            .collect::<Vec<_>>()
            .join("\n")
    }
    /// Reuses the parent's prefix through nested formatted spans, one run per open span.
    fn inline_with_prefix<'a>(
        &self,
        nodes: impl IntoIterator<Item = &'a Node>,
        style: Style<'_>,
        prefix: &str,
    ) -> String {
        let mut runs = vec![Run::new(nodes.into_iter().collect(), None)];
        loop {
            if let Some(rendered) = self.step(&mut runs, style, prefix) {
                return rendered;
            }
        }
    }
    /// Renders the next node of the innermost run, or closes the run when it is exhausted;
    /// the outermost run's text is the result.
    fn step(&self, runs: &mut Vec<Run<'_>>, style: Style<'_>, prefix: &str) -> Option<String> {
        let Some(run) = runs.last_mut() else {
            return Some(String::new());
        };
        let Some(node) = run.nodes.next() else {
            return self.close(runs, prefix);
        };
        if let Some((children, wrap)) = span(node) {
            runs.push(Run::new(children.iter().collect(), Some(wrap)));
        } else {
            let (text, restore) = self.leaf(&node.kind, style);
            run.result.push_str(&text);
            if restore {
                run.result.push_str(prefix);
            }
        }
        None
    }
    /// Pops the exhausted innermost run and styles it into its parent; the outermost run's
    /// text is returned.
    fn close(&self, runs: &mut Vec<Run<'_>>, prefix: &str) -> Option<String> {
        let Run {
            mut result, wrap, ..
        } = runs.pop()?;
        if !prefix.is_empty() {
            while result.ends_with(prefix) {
                result.truncate(result.len() - prefix.len());
            }
        }
        let (Some(wrap), Some(parent)) = (wrap, runs.last_mut()) else {
            return Some(result);
        };
        parent.result.push_str(&self.wrapped(&wrap, &result));
        parent.result.push_str(prefix);
        None
    }
    /// Renders one node that holds no nested inline children; the flag asks for the parent
    /// prefix to be restored after it.
    fn leaf(&self, kind: &Kind, style: Style<'_>) -> (String, bool) {
        match kind {
            Kind::Text(text) => (self.plain_inline(&text.decoded, style), false),
            Kind::Html(text)
            | Kind::HtmlBlock(text)
            | Kind::Image(text, _)
            | Kind::Heading(_, _, text)
            | Kind::Table(super::nodes::Table { authored: text, .. }) => {
                (self.plain_inline(text, style), false)
            }
            Kind::Quote(_, text) => (self.plain_inline(&text.to_owned_string(), style), false),
            Kind::Code(text) => ((self.theme.code)(&self.clean(text)), true),
            Kind::Break => ("\n".to_owned(), false),
            _ => (String::new(), false),
        }
    }
    /// Applies a formatted span's own styling to its rendered children.
    fn wrapped(&self, wrap: &Wrap<'_>, content: &str) -> String {
        match wrap {
            Wrap::Italic => (self.theme.italic)(content),
            Wrap::Bold => (self.theme.bold)(content),
            Wrap::Strikethrough => (self.theme.strikethrough)(content),
            Wrap::Link { label, href } => self.link(content, label, href),
        }
    }
    /// Selects clickable transport or the printed destination suffix.
    fn link(&self, content: &str, label: &str, href: &str) -> String {
        let label = self.clean(label);
        let href = self.clean(href);
        let href = href.as_ref();
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
