//! Terminal block formatting.
use super::{
    Renderer,
    inline::Style,
    nodes::{Kind, Node},
};

/// One pending step of a block walk, innermost last.
enum Step<'a> {
    /// Format the remaining nodes of one sibling run.
    Nodes(Run<'a>),
    /// Style and border the rows a quote's children produced.
    Quote {
        /// First row belonging to the quote's children.
        start: usize,
        /// Width requested by the enclosing block.
        width: usize,
    },
    /// Emit the blank row separating two blocks.
    Separation,
}

/// One sibling run's formatting inputs.
#[derive(Clone, Copy)]
struct Run<'a> {
    /// Siblings still to format.
    nodes: &'a [Node],
    /// Available width at this quote depth.
    width: usize,
    /// Whether the run is inside a quote.
    quoted: bool,
}

impl Renderer<'_> {
    /// Formats blocks with sibling-sensitive spacing.
    pub(super) fn blocks(&self, nodes: &[Node], width: usize) -> Vec<String> {
        let prefix = self.extract_prefix(Style::Quote(""));
        let mut rows = Vec::new();
        let mut pending = vec![Step::Nodes(Run {
            nodes,
            width,
            quoted: false,
        })];
        while let Some(step) = pending.pop() {
            match step {
                Step::Separation => rows.push(String::new()),
                Step::Quote { start, width } => self.quote(&mut rows, start, width, &prefix),
                Step::Nodes(run) => self.next_block(run, &prefix, &mut rows, &mut pending),
            }
        }
        rows
    }
    /// Emit one block and queue the steps that follow it.
    fn next_block<'a>(
        &self,
        run: Run<'a>,
        prefix: &str,
        rows: &mut Vec<String>,
        pending: &mut Vec<Step<'a>>,
    ) {
        let Some((node, rest)) = run.nodes.split_first() else {
            return;
        };
        if !rest.is_empty() {
            pending.push(Step::Nodes(Run { nodes: rest, ..run }));
        }
        if rest
            .first()
            .is_some_and(|next| separates(&node.kind, &next.kind))
        {
            pending.push(Step::Separation);
        }
        let style = if run.quoted {
            Style::Quote(prefix)
        } else {
            Style::Default
        };
        match &node.kind {
            Kind::Paragraph(children) => rows.push(self.inline(children, style)),
            Kind::Heading(level, children, _) => rows.push(self.heading(*level, children)),
            Kind::CodeBlock(text, info) => rows.extend(self.code_block(text, info.as_deref())),
            Kind::HtmlBlock(text) => rows.push(self.style(
                &self.clean(text.trim_matches(crate::text::utils::is_whitespace_scalar)),
                style,
            )),
            Kind::Quote(children, _) => {
                pending.push(Step::Quote {
                    start: rows.len(),
                    width: run.width,
                });
                pending.push(Step::Nodes(Run {
                    nodes: children,
                    width: quote_width(run.width),
                    quoted: true,
                }));
            }
            Kind::Rule => rows.push((self.theme.hr)(&"─".repeat(run.width.min(80)))),
            Kind::List(start, items) => rows.extend(self.list(*start, items, style)),
            Kind::Table(table) => rows.extend(self.table(table, run.width, style)),
            Kind::Gap => rows.push(String::new()),
            _ => {}
        }
    }
    /// Formats code through the supplied highlighter or plain code callback.
    pub(super) fn code_block(&self, text: &str, info: Option<&str>) -> Vec<String> {
        let text = self.clean(text.strip_suffix('\n').unwrap_or(text));
        let text = text.as_ref();
        let info = info.map(|info| self.clean(info));
        let info = info.as_deref();
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
    /// Styles quote children, reserving a two-cell border only when one content cell fits.
    fn quote(&self, rows: &mut Vec<String>, start: usize, width: usize, prefix: &str) {
        let bordered = width >= 3;
        let width = quote_width(width);
        let mut owned = rows.split_off(start);
        while owned.last().is_some_and(String::is_empty) {
            owned.pop();
        }
        rows.extend(
            owned
                .into_iter()
                .flat_map(|row| {
                    let row = row.replace("\x1b[0m", &format!("\x1b[0m{prefix}"));
                    let row = (self.theme.quote)(&(self.theme.italic)(&row));
                    crate::wrap_text_with_ansi(&row, width)
                })
                .map(|row| {
                    if bordered {
                        format!("{}{row}", (self.theme.quote_border)("│ "))
                    } else {
                        row
                    }
                }),
        );
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

/// Width available to a quote's children at one level.
fn quote_width(width: usize) -> usize {
    if width >= 3 { width - 2 } else { width.max(1) }
}

/// Chooses spacing from the current and next block kinds.
fn separates(current: &Kind, next: &Kind) -> bool {
    match current {
        Kind::Paragraph(_) => !matches!(next, Kind::Gap | Kind::List(..)),
        Kind::Heading(..) | Kind::CodeBlock(..) | Kind::Quote(..) | Kind::Rule | Kind::Table(_) => {
            !matches!(next, Kind::Gap)
        }
        _ => false,
    }
}
