//! List admission and item continuation framing.
use crate::{
    Kind, MarkedOptions, Node,
    inline::ScopeKey,
    lexer::Lexer,
    rules,
    source::{ScopeSource, Segmented, Source},
    tokenizer::{capture, node},
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
    /// Final promotion policy, applied when child nodes become observable.
    loose: bool,
}

impl ListState {
    /// Transfer each child's single output into its final item without cloning it.
    pub(crate) fn finish(mut self, items: &mut [Node], scopes: &mut HashMap<ScopeKey, Segmented>) {
        for (item, node) in self
            .completed
            .iter_mut()
            .chain(std::iter::once(&mut self.item))
            .zip(items)
        {
            item.output.complete();
            scopes.extend(std::mem::take(&mut item.output.scopes));
            if let Kind::Item(children) = &mut node.kind {
                *children = std::mem::take(&mut item.output.nodes);
            }
            if self.loose {
                loosen(node);
            }
        }
    }
}

impl<E> Lexer<'_, E> {
    /// Admit a list using bounded physical-line operands.
    pub(crate) fn list(&mut self, source: &Source<'_>) -> Option<(usize, Node, Box<ListState>)> {
        let first_line = source.text.split_inclusive('\n').next()?;
        #[cfg(test)]
        crate::tests::block_operand(first_line);
        rules::BLOCK_LIST.as_ref()?.find(first_line)?;
        let first = marker(first_line)?;
        let item = ItemLines::new(
            &source.select(std::iter::once(0..first.line)),
            &first,
            self.options,
        );
        let offset = first.line;
        let state = Box::new(ListState {
            first,
            item,
            item_start: 0,
            completed: Vec::new(),
            loose: false,
        });
        self.list_tail(
            &source.select(std::iter::once(0..source.text.len())).into(),
            state,
            offset,
        )
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
        let mut loose = false;
        for item in &mut state.completed {
            let (node, item_loose) = self.list_item(item, true)?;
            loose |= item_loose;
            items.push(node);
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
            let (item, item_loose) = self.list_item(&mut state.item, true)?;
            loose |= item_loose;
            items.push(item);
            state.item_start = offset;
            let previous =
                std::mem::replace(&mut state.item, ItemLines::new(&line, &next, self.options));
            state.completed.push(previous);
            offset += next.line;
        }
        let (item, item_loose) = self.list_item(&mut state.item, false)?;
        items.push(item);
        state.loose = loose || item_loose;
        let end = state.item_start + state.item.raw_end();
        let raw = source.select(0..end);
        let node = Node {
            range: raw.native(&(0..end))?,
            authored: raw.mapped(),
            kind: Kind::List(state.first.start, items),
        };
        Some((end, node, state))
    }

    /// Replay child admission while retaining its output for final publication.
    fn list_item(&mut self, item: &mut ItemLines, following: bool) -> Option<(Node, bool)> {
        let (mut body, task) = task(item.body().materialize());
        if !following {
            body.truncate(body.text.trim_end_matches(is_whitespace_scalar).len());
        }
        self.item_blocks(&body, &mut item.output);
        if let Some(checked) = task {
            item.output
                .nodes
                .insert(0, node(&body, 0..0, Kind::TaskListMarker(checked)));
        }
        let raw = item.raw();
        let loose = following && item.blank_end()
            || item.output.nodes.iter().any(|node| {
                matches!(node.kind, Kind::Gap)
                    && node
                        .authored
                        .chunks()
                        .flat_map(str::bytes)
                        .filter(|byte| *byte == b'\n')
                        .count()
                        > 1
            });
        Some((
            Node {
                authored: raw.mapped(),
                range: raw.native(&(0..raw.len()))?,
                kind: Kind::Item(Vec::new()),
            },
            loose,
        ))
    }
}

/// Task markers are consumed before the item's ordinary inline grammar.
fn task(source: ScopeSource) -> (ScopeSource, Option<bool>) {
    let bytes = source.text.as_bytes();
    if bytes.len() >= 4
        && bytes[0] == b'['
        && bytes[2] == b']'
        && bytes[3] == b' '
        && matches!(bytes[1], b' ' | b'x' | b'X')
    {
        let end = 3 + source.text[3..]
            .bytes()
            .take_while(|byte| *byte == b' ')
            .count();
        let checked = bytes[1] != b' ';
        (
            source.select(std::iter::once(end..source.text.len())),
            Some(checked),
        )
    } else {
        (source, None)
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
