//! Semantic nested-list rows and item-specific block fallback.
use super::{
    Renderer,
    inline::Style,
    parse::{Kind, Node},
};

/// Whether a row already carries its complete nested indentation.
enum Row {
    /// Content receiving the current item's marker or continuation indent.
    Content(String),
    /// A nested list's fully indented row.
    Nested(String),
}
impl Renderer<'_> {
    /// Renders markers and semantic nested rows in item order.
    pub(super) fn list(
        &self,
        start: Option<u64>,
        items: &[Node],
        depth: usize,
        style: Style<'_>,
    ) -> Vec<String> {
        let mut lines = Vec::new();
        let indent = "  ".repeat(depth);
        for (index, item) in items.iter().enumerate() {
            let marker = start.map_or_else(
                || "- ".to_owned(),
                |start| format!("{}. ", start + index as u64),
            );
            let Kind::Item(children) = &item.kind else {
                continue;
            };
            let rows = self.item(children, depth, style);
            if rows.is_empty() {
                lines.push(format!("{indent}{}", (self.theme.list_bullet)(&marker)));
            }
            lines.extend(rows.into_iter().enumerate().map(|(index, row)| match row {
                Row::Nested(text) => text,
                Row::Content(text) if index == 0 => {
                    format!("{indent}{}{text}", (self.theme.list_bullet)(&marker))
                }
                Row::Content(text) => format!("{indent}  {text}"),
            }));
        }
        lines
    }
    /// Renders item content without ordinary block spacing.
    fn item(&self, nodes: &[Node], depth: usize, style: Style<'_>) -> Vec<Row> {
        let mut rows = Vec::new();
        let mut inline = Vec::new();
        for node in nodes {
            if matches!(
                node.kind,
                Kind::Text(_)
                    | Kind::Break
                    | Kind::Code(_)
                    | Kind::Emphasis(_)
                    | Kind::Strong(_)
                    | Kind::Strike(_)
                    | Kind::Link(..)
                    | Kind::Image(_)
                    | Kind::Html(_)
                    | Kind::HtmlBlock(_)
            ) {
                inline.push(node);
                continue;
            }
            if !inline.is_empty() {
                rows.push(Row::Content(self.inline(inline.iter().copied(), style)));
                inline.clear();
            }
            match &node.kind {
                Kind::List(start, items) => rows.extend(
                    self.list(*start, items, depth + 1, style)
                        .into_iter()
                        .map(Row::Nested),
                ),
                Kind::Paragraph(children) => rows.push(Row::Content(self.inline(children, style))),
                Kind::Heading(..) | Kind::Quote(..) => {
                    rows.push(Row::Content(self.inline(std::iter::once(node), style)));
                }
                Kind::CodeBlock(text, info) => rows.extend(
                    self.code_block(text, info.as_deref())
                        .into_iter()
                        .map(Row::Content),
                ),
                _ => {}
            }
        }
        if !inline.is_empty() {
            rows.push(Row::Content(self.inline(inline.iter().copied(), style)));
        }
        rows
    }
}
