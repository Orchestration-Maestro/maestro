//! List admission and item continuation framing.
use crate::{
    Kind, MarkedOptions, Node,
    inline::ScopeKey,
    lexer::Lexer,
    rules,
    source::{ScopeSource, Segmented},
    tokenizer::capture,
};
use maestro_text::is_whitespace_scalar;
use std::collections::HashMap;
mod state;
use state::ItemLines;

/// Marker identity and indentation at the start of one list item.
struct Marker {
    /// Ordered value, absent for bullets.
    start: Option<u64>,
    /// Delimiter or unordered bullet.
    bullet: u8,
    /// Complete first-line extent.
    line: usize,
    /// End of the marker before first-line whitespace.
    prefix: usize,
    /// Continuation indentation.
    indent: usize,
    /// Leading first-line tabs expanded before measuring content.
    tabs: usize,
}

impl Marker {
    /// Keep orderedness and the selected bullet or delimiter within one list.
    fn follows(&self, first: &Self) -> bool {
        self.start.is_some() == first.start.is_some() && self.bullet == first.bullet
    }
}

/// Recognize a marker without treating a horizontal rule as an item.
fn marker(source: &str) -> Option<Marker> {
    #[cfg(test)]
    crate::tests::block_operand(source);
    let matched = rules::LIST_ITEM.as_ref()?.find(source)?;
    #[cfg(test)]
    crate::tests::block_operand(source);
    if rules::BLOCK_HR.as_ref()?.find(source).is_some() {
        return None;
    }
    let raw = capture(source, &matched, 1);
    let bullet = *raw.as_bytes().last()?;
    let ordered = raw.trim_start().len() > 1;
    let start = ordered
        .then(|| raw.trim().trim_end_matches(['.', ')']).parse().ok())
        .flatten();
    let line = matched.end();
    let rest = &source[raw.len()..line];
    let blank = rest.trim_matches(is_whitespace_scalar).is_empty();
    let spaces = rest.bytes().take_while(|byte| *byte == b' ').count();
    let indent = if blank || spaces > 4 { 1 } else { spaces };
    let tabs = if blank {
        0
    } else {
        rest.bytes().take_while(|byte| *byte == b'\t').count()
    };
    Some(Marker {
        start,
        bullet,
        line,
        prefix: raw.len(),
        indent: raw.len() + indent,
        tabs,
    })
}

/// Item-local lookahead rules, prepared once for the selected indentation.
struct ItemRules {
    /// Fences, headings, neighboring markers, rules and HTML prefixes.
    stops: [Option<regress::Regex>; 5],
}
impl ItemRules {
    /// Bind the five shared prefix factories to this item's indentation.
    fn new(indent: usize, options: MarkedOptions) -> Self {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.list_rules += 5);
        let prefix = format!("^ {{0,{}}}", indent.saturating_sub(1).min(3));
        let factories = [
            "(?:```|~~~)",
            "#",
            r"(?:[*+-]|\d{1,9}[.)])((?:[ \t][^\n]*)?(?:\n|$))",
            r"((?:- *){3,}|(?:_ *){3,}|(?:\* *){3,})(?:\n+|$)",
            if options == (MarkedOptions::Browser { escape_html: true }) {
                "<!--"
            } else {
                "<(?:[a-z].*>|!--)"
            },
        ];
        let stops =
            factories.map(|rule| regress::Regex::with_flags(&format!("{prefix}{rule}"), "i").ok());
        Self { stops }
    }

    /// A current line can end framing before it is dedented.
    fn interrupts(&self, line: &str) -> bool {
        self.stops.iter().flatten().any(|rule| {
            #[cfg(test)]
            crate::tests::block_operand(line);
            rule.find(line).is_some()
        })
    }

    /// A previous code line or block opener cannot admit another lazy line.
    fn ends_lazy(&self, line: &str) -> bool {
        let code = line
            .find(|scalar| !matches!(scalar, ' ' | '\t'))
            .is_some_and(|end| {
                line[..end]
                    .chars()
                    .map(|scalar| if scalar == '\t' { 4 } else { 1 })
                    .sum::<usize>()
                    >= 4
            });
        code || [0, 1, 3]
            .into_iter()
            .filter_map(|index| self.stops[index].as_ref())
            .any(|rule| {
                #[cfg(test)]
                crate::tests::block_operand(line);
                rule.find(line).is_some()
            })
    }
}

/// Retained list identity and trailing item's admission history.
pub(crate) struct ListState {
    /// The first marker fixes orderedness and delimiter identity.
    first: Marker,
    /// Framing and source of the final item, retained until the next boundary.
    item: ItemLines,
    /// Logical start of the final item within its list operand.
    item_start: usize,
    /// Completed item admission states, preserving callback replay order.
    completed: Vec<ItemLines>,
}

impl ListState {
    /// Complete the trailing item and admit the next marker's first line; nested items
    /// recurse through the list driver, so the replaced states stay out of its frame.
    fn begin(&mut self, line: &ScopeSource, next: &Marker, options: MarkedOptions) {
        let previous = std::mem::replace(&mut self.item, ItemLines::new(line, next, options));
        self.completed.push(previous);
    }

    /// Transfer each child's single output into its final item without cloning it.
    pub(crate) fn finish(mut self, items: &mut [Node], scopes: &mut HashMap<ScopeKey, Segmented>) {
        let following = self.completed.len();
        let mut outputs = Vec::with_capacity(following + 1);
        let mut loose = false;
        for (index, item) in self
            .completed
            .iter_mut()
            .chain(std::iter::once(&mut self.item))
            .enumerate()
        {
            let output = item.child.finish(task(item.body()).1);
            loose |= index < following && item.blank_end()
                || output.nodes.iter().any(|node| {
                    matches!(node.kind, Kind::Gap)
                        && node
                            .authored
                            .chunks()
                            .flat_map(str::bytes)
                            .filter(|byte| *byte == b'\n')
                            .count()
                            > 1
                });
            outputs.push(output);
        }
        for (output, node) in outputs.into_iter().zip(items) {
            scopes.extend(output.scopes);
            if let Kind::Item(children) = &mut node.kind {
                *children = output.nodes;
            }
            if loose {
                loosen(node);
            }
        }
    }
}

impl<E> Lexer<'_, E> {
    /// Admit a list using bounded physical-line operands.
    pub(crate) fn list(&mut self, source: &Segmented) -> Option<(usize, Node, Box<ListState>)> {
        let first = source.first_line();
        let first_line = first.text.split_inclusive('\n').next()?;
        #[cfg(test)]
        crate::tests::block_operand(first_line);
        rules::BLOCK_LIST.as_ref()?.find(first_line)?;
        let first = marker(first_line)?;
        let item = ItemLines::new(
            &source.select(0..first.line).materialize(),
            &first,
            self.options,
        );
        let offset = first.line;
        let state = Box::new(ListState {
            first,
            item,
            item_start: 0,
            completed: Vec::new(),
        });
        self.list_tail(source, state, offset)
    }

    /// Resume the same item's source and history after its enclosing quote grows.
    pub(crate) fn resume_list(
        &mut self,
        source: &Segmented,
        mut state: Box<ListState>,
    ) -> Option<(usize, Node, Box<ListState>)> {
        let start = state.item_start;
        let offset = state.item.resume(source, start);
        self.list_tail(source, state, offset)
    }

    /// Continue item admission without revisiting completed physical lines.
    fn list_tail(
        &mut self,
        source: &Segmented,
        mut state: Box<ListState>,
        mut offset: usize,
    ) -> Option<(usize, Node, Box<ListState>)> {
        let mut items = Vec::with_capacity(state.completed.len() + 1);
        for item in &mut state.completed {
            items.push(self.list_item(item, true)?);
        }
        let mut pending = crate::source::Cursor::default();
        pending.segmented(source.select(offset..source.len()));
        while let Some(line) = pending.line() {
            let line = line.materialize();
            if state.item.append(&line) {
                offset += line.text.len();
                continue;
            }
            let Some(next) = marker(&line.text).filter(|next| next.follows(&state.first)) else {
                break;
            };
            items.push(self.list_item(&mut state.item, true)?);
            state.item_start = offset;
            state.begin(&line, &next, self.options);
            offset += next.line;
        }
        items.push(self.list_item(&mut state.item, false)?);
        let end = state.item_start + state.item.raw_end();
        let raw = source.select(0..end);
        let node = Node {
            range: raw.native(&(0..end))?,
            authored: raw.mapped(),
            kind: Kind::List(state.first.start, items),
        };
        Some((end, node, state))
    }

    /// Admit the item's grown child operand and publish its provisional wrapper.
    fn list_item(&mut self, item: &mut ItemLines, following: bool) -> Option<Node> {
        let (mut body, _) = task(item.body());
        let prefix = item.body().len() - body.len();
        if !following {
            let trimmed = item.body().len() - item.body_end();
            body.truncate(body.len().saturating_sub(trimmed));
        }
        self.item_blocks(&body, prefix, &mut item.child);
        let raw = item.raw();
        Some(Node {
            authored: raw.mapped(),
            range: raw.native(&(0..raw.len()))?,
            kind: Kind::Item(Vec::new()),
        })
    }
}

/// Task markers are consumed before the item's ordinary inline grammar.
fn task(source: &Segmented) -> (Segmented, Option<bool>) {
    let first = source.first_line();
    let bytes = first.text.as_bytes();
    if bytes.len() >= 4
        && bytes[0] == b'['
        && bytes[2] == b']'
        && bytes[3] == b' '
        && matches!(bytes[1], b' ' | b'x' | b'X')
    {
        let end = 3 + first.text[3..]
            .bytes()
            .take_while(|byte| *byte == b' ')
            .count();
        let checked = bytes[1] != b' ';
        (source.select(end..source.len()), Some(checked))
    } else {
        (source.clone(), None)
    }
}

/// Loose items promote line text without changing genuine paragraphs.
fn loosen(item: &mut Node) {
    let Kind::Item(children) = &mut item.kind else {
        return;
    };
    for child in children {
        if let Kind::Inline(content) = &mut child.kind {
            child.kind = Kind::Paragraph(std::mem::take(content));
        }
    }
}
