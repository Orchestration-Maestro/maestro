//! Terminal block formatting.
use super::{
    Renderer,
    inline::Style,
    parse::{Kind, Node},
};

impl Renderer<'_> {
    /// Formats blocks with sibling-sensitive spacing.
    pub(super) fn blocks(&self, nodes: &[Node], width: usize, style: Style<'_>) -> Vec<String> {
        let mut rows = Vec::new();
        for (index, node) in nodes.iter().enumerate() {
            match &node.kind {
                Kind::Paragraph(children) => rows.push(self.inline(children, style)),
                Kind::Heading(level, children, _) => rows.push(self.heading(*level, children)),
                Kind::CodeBlock(text, info) => rows.extend(self.code_block(text, info.as_deref())),
                Kind::HtmlBlock(text) => rows.push(self.style(
                    text.trim_matches(crate::text::utils::is_whitespace_scalar),
                    Style::Default,
                )),
                Kind::Quote(children, _) => rows.extend(self.quote(children, width)),
                Kind::Rule => rows.push((self.theme.hr)(&"─".repeat(width.min(80)))),
                Kind::List(start, items) => rows.extend(self.list(*start, items, 0, style)),
                Kind::Gap => rows.push(String::new()),
                _ => {}
            }
            if nodes
                .get(index + 1)
                .is_some_and(|next| separates(&node.kind, &next.kind))
            {
                rows.push(String::new());
            }
        }
        rows
    }
    /// Formats code through the supplied highlighter or plain code callback.
    pub(super) fn code_block(&self, text: &str, info: Option<&str>) -> Vec<String> {
        let indent = self.theme.code_block_indent.as_deref().unwrap_or("  ");
        let mut rows = vec![(self.theme.code_block_border)(&format!(
            "```{}",
            info.unwrap_or_default()
        ))];
        let code: Vec<String> = match &self.theme.highlight_code {
            Some(highlight) => highlight(text, info),
            None => text
                .split('\n')
                .map(|line| (self.theme.code_block)(line))
                .collect(),
        };
        rows.extend(code.into_iter().map(|line| format!("{indent}{line}")));
        rows.push((self.theme.code_block_border)("```"));
        rows
    }
    /// Styles quote children and wraps them inside the two-cell border.
    fn quote(&self, nodes: &[Node], width: usize) -> Vec<String> {
        let width = width.saturating_sub(2).max(1);
        let prefix = self.extract_prefix(Style::Quote(""));
        let mut rows = self.blocks(nodes, width, Style::Quote(&prefix));
        while rows.last().is_some_and(String::is_empty) {
            rows.pop();
        }
        rows.into_iter()
            .flat_map(|row| {
                let row = row.replace("\x1b[0m", &format!("\x1b[0m{prefix}"));
                let row = (self.theme.quote)(&(self.theme.italic)(&row));
                crate::wrap_text_with_ansi(&row, width)
                    .into_iter()
                    .map(|row| format!("{}{row}", (self.theme.quote_border)("│ ")))
            })
            .collect()
    }
    /// Formats a heading with hashes only for levels three and higher.
    fn heading(&self, level: usize, children: &[Node]) -> String {
        let style = Style::Heading(level);
        let content = self.inline(children, style);
        let prefix = if level >= 3 {
            self.style(&format!("{} ", "#".repeat(level)), style)
        } else {
            String::new()
        };
        format!("{prefix}{content}")
    }
}

/// Chooses spacing from the current and next block kinds.
fn separates(current: &Kind, next: &Kind) -> bool {
    match current {
        Kind::Paragraph(_) => !matches!(next, Kind::Gap | Kind::List(..)),
        Kind::Heading(..) | Kind::CodeBlock(..) | Kind::Quote(..) | Kind::Rule => {
            !matches!(next, Kind::Gap)
        }
        _ => false,
    }
}
