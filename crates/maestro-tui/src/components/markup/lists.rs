//! Semantic nested-list rows and item-specific block fallback.
use super::{
    Renderer,
    inline::Style,
    nodes::{Kind, Node},
};

/// Whether a row already carries its complete nested indentation.
enum Row {
    /// Content receiving the current item's marker or continuation indent.
    Content(String),
    /// A nested list's fully indented row.
    Nested(String),
}

/// One list whose items are being formatted.
struct Listing<'a> {
    /// Remaining items.
    items: std::slice::Iter<'a, Node>,
    /// Ordered start, if any.
    start: Option<u64>,
    /// Position of the next item.
    index: usize,
    /// Nesting depth supplying indentation.
    depth: usize,
    /// Rows already formatted for finished items.
    lines: Vec<String>,
}

/// One item's children being formatted.
struct Entry<'a> {
    /// Remaining children.
    children: std::slice::Iter<'a, Node>,
    /// Consecutive inline children awaiting one content row.
    inline: Vec<&'a Node>,
    /// Rows produced for this item, in authored order.
    rows: Vec<Row>,
    /// Ordered start of the enclosing list.
    start: Option<u64>,
    /// This item's position in its list.
    index: usize,
    /// Enclosing list's nesting depth.
    depth: usize,
}

/// One pending level of a list walk, innermost last.
enum Level<'a> {
    /// A list formatting its items.
    List(Listing<'a>),
    /// An item formatting its children.
    Item(Entry<'a>),
}

impl Renderer<'_> {
    /// Renders markers and semantic nested rows in item order.
    pub(super) fn list(&self, start: Option<u64>, items: &[Node], style: Style<'_>) -> Vec<String> {
        let mut levels = vec![Level::List(Listing {
            items: items.iter(),
            start,
            index: 0,
            depth: 0,
            lines: Vec::new(),
        })];
        let mut done = None;
        while let Some(level) = levels.pop() {
            match level {
                Level::List(listing) => step_list(listing, &mut levels, &mut done),
                Level::Item(entry) => self.step_item(entry, style, &mut levels),
            }
        }
        done.unwrap_or_default()
    }
    /// Admit one item's next child, or format a finished item into its list.
    fn step_item<'a>(&self, mut entry: Entry<'a>, style: Style<'_>, levels: &mut Vec<Level<'a>>) {
        let Some(node) = entry.children.next() else {
            return self.finish_item(entry, style, levels);
        };
        if is_inline(&node.kind) {
            entry.inline.push(node);
            levels.push(Level::Item(entry));
            return;
        }
        self.flush(&mut entry, style);
        match &node.kind {
            Kind::List(start, items) => {
                let depth = entry.depth + 1;
                levels.push(Level::Item(entry));
                levels.push(Level::List(Listing {
                    items: items.iter(),
                    start: *start,
                    index: 0,
                    depth,
                    lines: Vec::new(),
                }));
            }
            Kind::Paragraph(children) | Kind::Inline(children) => {
                entry.rows.push(Row::Content(self.inline(children, style)));
                levels.push(Level::Item(entry));
            }
            Kind::Heading(..) | Kind::Quote(..) | Kind::Table(_) => {
                let text = self.inline(std::iter::once(node), style);
                entry
                    .rows
                    .extend(Some(text).filter(|text| !text.is_empty()).map(Row::Content));
                levels.push(Level::Item(entry));
            }
            Kind::CodeBlock(text, info) => {
                entry.rows.extend(
                    self.code_block(text, info.as_deref())
                        .into_iter()
                        .map(Row::Content),
                );
                levels.push(Level::Item(entry));
            }
            _ => levels.push(Level::Item(entry)),
        }
    }
    /// Emit the pending inline run of one item as its content row.
    fn flush(&self, entry: &mut Entry<'_>, style: Style<'_>) {
        if !entry.inline.is_empty() {
            let row = self.inline(entry.inline.iter().copied(), style);
            entry.rows.push(Row::Content(row));
            entry.inline.clear();
        }
    }
    /// Mark and indent one finished item's rows into its enclosing list.
    fn finish_item<'a>(&self, mut entry: Entry<'a>, style: Style<'_>, levels: &mut [Level<'a>]) {
        self.flush(&mut entry, style);
        let marker = entry.start.map_or_else(
            || "- ".to_owned(),
            |start| format!("{}. ", start + entry.index as u64),
        );
        let indent = "  ".repeat(entry.depth);
        let Some(Level::List(listing)) = levels.last_mut() else {
            return;
        };
        if entry.rows.is_empty() {
            listing
                .lines
                .push(format!("{indent}{}", (self.theme.list_bullet)(&marker)));
        }
        listing.lines.extend(
            entry
                .rows
                .into_iter()
                .enumerate()
                .map(|(index, row)| match row {
                    Row::Nested(text) => text,
                    Row::Content(text) if index == 0 => {
                        format!("{indent}{}{text}", (self.theme.list_bullet)(&marker))
                    }
                    Row::Content(text) => format!("{indent}  {text}"),
                }),
        );
    }
}

/// Admit one list's next item, or hand a finished list to its enclosing item.
fn step_list<'a>(
    mut listing: Listing<'a>,
    levels: &mut Vec<Level<'a>>,
    done: &mut Option<Vec<String>>,
) {
    let Some(item) = listing.items.next() else {
        match levels.last_mut() {
            Some(Level::Item(parent)) => {
                parent
                    .rows
                    .extend(listing.lines.into_iter().map(Row::Nested));
            }
            _ => *done = Some(listing.lines),
        }
        return;
    };
    let Kind::Item(children) = &item.kind else {
        return levels.push(Level::List(listing));
    };
    let entry = Entry {
        children: children.iter(),
        inline: Vec::new(),
        rows: Vec::new(),
        start: listing.start,
        index: listing.index,
        depth: listing.depth,
    };
    listing.index += 1;
    levels.push(Level::List(listing));
    levels.push(Level::Item(entry));
}

/// Whether a block belongs to the current item's inline run.
fn is_inline(kind: &Kind) -> bool {
    matches!(
        kind,
        Kind::Text(_)
            | Kind::Break
            | Kind::Code(_)
            | Kind::Emphasis(_)
            | Kind::Strong(_)
            | Kind::Strike(_)
            | Kind::Link(..)
            | Kind::Image(..)
            | Kind::Html(_)
            | Kind::HtmlBlock(_)
    )
}
