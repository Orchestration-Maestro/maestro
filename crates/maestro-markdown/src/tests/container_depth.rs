//! Deep container nesting, its recorded documents and record teardown.
use super::{REDUCED_STACK, WORKER_STACK, nodes::children, on_small_stack, on_stack};
use crate::{Document, InlineContext, Kind, MappedText, MarkedOptions, Node, parse_with_options};

/// Caller grammars every depth row covers.
const OPTIONS: [MarkedOptions; 5] = [
    MarkedOptions::Terminal,
    MarkedOptions::Browser { escape_html: false },
    MarkedOptions::Browser { escape_html: true },
    MarkedOptions::Export,
    MarkedOptions::Documentation,
];

/// Container chain length and complete node count of one document.
fn shape(document: &Document) -> (usize, usize) {
    let mut containers = 0;
    let mut nodes = 0;
    let mut pending: Vec<(usize, &Node)> = document.nodes.iter().map(|node| (1, node)).collect();
    while let Some((depth, node)) = pending.pop() {
        nodes += 1;
        if matches!(node.kind, Kind::Quote(..) | Kind::List(..) | Kind::Item(..)) {
            containers = containers.max(depth);
        }
        pending.extend(children(&node.kind).map(|child| (depth + 1, child)));
    }
    (containers, nodes)
}

/// Short tag for one node kind in a recorded document.
fn tag(kind: &Kind) -> &'static str {
    match kind {
        Kind::CustomBlock(..) => "custom",
        Kind::Atom(_) => "atom",
        Kind::Paragraph(_) => "paragraph",
        Kind::Inline(_) => "inline",
        Kind::Heading(..) => "heading",
        Kind::List(None, _) => "list",
        Kind::List(Some(_), _) => "ordered",
        Kind::Item(_) => "item",
        Kind::HtmlBlock(_) => "html-block",
        Kind::Html(_) => "html",
        Kind::Rule => "rule",
        Kind::Gap => "gap",
        Kind::Quote(..) => "quote",
        Kind::Text(_) => "text",
        Kind::CodeBlock(..) => "code-block",
        Kind::Link(..) => "link",
        Kind::Image(..) => "image",
        Kind::TaskListMarker(checked) => {
            if *checked {
                "task-checked"
            } else {
                "task"
            }
        }
        Kind::Strong(_) => "strong",
        Kind::Strike(_) => "strike",
        Kind::Emphasis(_) => "emphasis",
        Kind::Code(_) => "code",
        Kind::Break => "break",
        Kind::Table(_) => "table",
    }
}

/// Leaf spelling recorded with its node, so the witness sees decoded content.
fn leaf(kind: &Kind) -> String {
    match kind {
        Kind::Text(text) => format!(" {:?}", text.decoded),
        Kind::Code(code) | Kind::CodeBlock(code, _) => format!(" {code:?}"),
        Kind::Html(html) | Kind::HtmlBlock(html) => format!(" {html:?}"),
        Kind::Heading(level, _, label) => format!(" {level} {label:?}"),
        Kind::Link(_, label, destination, _) => format!(" {label:?} {destination:?}"),
        Kind::CustomBlock(kind, _) => format!(" {kind:?}"),
        _ => String::new(),
    }
}

/// One line per node, indented by depth, naming kind, extent, authored spelling and leaves,
/// followed by one line per retained inline scope.
pub(super) fn recorded(document: &Document) -> String {
    let mut lines = Vec::new();
    let mut pending: Vec<(usize, &Node)> =
        document.nodes.iter().rev().map(|node| (0, node)).collect();
    while let Some((depth, node)) = pending.pop() {
        lines.push(format!(
            "{}{} {}..{} {:?}{}{}",
            "  ".repeat(depth),
            tag(&node.kind),
            node.range.start,
            node.range.end,
            node.authored.to_owned_string(),
            container(&node.kind),
            leaf(&node.kind)
        ));
        let kids: Vec<&Node> = children(&node.kind).collect();
        pending.extend(kids.into_iter().rev().map(|child| (depth + 1, child)));
    }
    let mut scopes: Vec<_> = document.scopes.iter().collect();
    scopes.sort_by(|left, right| left.0.cmp(right.0));
    for (key, source) in scopes {
        lines.push(format!(
            "scope {}..{} label={} {:?}",
            key.0, key.1, key.2, &*source.text
        ));
    }
    lines.join("\n")
}

/// Quote body and ordered start that a container owns beyond its children.
fn container(kind: &Kind) -> String {
    match kind {
        Kind::Quote(_, body) => format!(" body={:?}", body.to_owned_string()),
        Kind::List(Some(start), _) => format!(" start={start}"),
        _ => String::new(),
    }
}

/// Assert one marker shape's container depth and node count for every grammar.
fn depth_rows(stack: usize, source: fn() -> String, expected: (usize, usize)) {
    for options in OPTIONS {
        let measured = on_stack(stack, move || shape(&parse_with_options(source(), options)));
        assert_eq!(measured, expected, "{options:?}");
    }
}

#[test]
fn deep_quote_markers_parse_on_a_small_stack() {
    depth_rows(WORKER_STACK, || ">".repeat(3_000), (3_000, 3_000));
}

#[test]
fn deep_quote_list_markers_parse_on_a_small_stack() {
    depth_rows(WORKER_STACK, || "> - ".repeat(500), (1_500, 1_500));
}

#[test]
fn deep_dash_markers_stay_one_thematic_break() {
    for options in OPTIONS {
        let measured =
            on_small_stack(move || shape(&parse_with_options("- ".repeat(100_000), options)));
        assert_eq!(measured, (0, 1), "{options:?}");
    }
    for count in [3, 10, 1_000] {
        let document = parse_with_options("- ".repeat(count), MarkedOptions::Terminal);
        assert_eq!(document.nodes.len(), 1, "{count}");
        assert!(matches!(document.nodes[0].kind, Kind::Rule));
    }
    let empty = shape(&parse_with_options(
        "- ".to_owned(),
        MarkedOptions::Terminal,
    ));
    assert_eq!(empty, (0, 2));
    let nested = shape(&parse_with_options(
        "- - ".to_owned(),
        MarkedOptions::Terminal,
    ));
    assert_eq!(nested, (4, 4));
}

/// The repeated resumption message: each repeat resumes the list a quote left, three
/// container levels deeper.
fn resumption_message(repeats: usize) -> String {
    let mut source = String::from("> intro\nlazy\n");
    for _ in 0..repeats {
        source.push_str("> - a\n    cont\n> \n> para\n> \n");
    }
    source
}

/// Container depth and node count of the repeated resumption message.
fn resumption(repeats: usize, options: MarkedOptions) -> (usize, usize) {
    let case = move || shape(&parse_with_options(resumption_message(repeats), options));
    on_small_stack(case)
}

#[test]
fn repeated_quote_list_resumption_keeps_reference_depth() {
    // Every grammar resumes to the same container depth, and the terminal grammar keeps the
    // node count the reference gives each depth.
    for options in OPTIONS {
        let shallow = resumption(64, options);
        assert_eq!(shallow.0, 193, "{options:?} at 64");
        let deep = resumption(128, options);
        assert_eq!(deep.0, 385, "{options:?} at 128");
        if matches!(options, MarkedOptions::Terminal) {
            assert_eq!(shallow, (193, 578), "{options:?} at 64");
            assert_eq!(deep, (385, 1_154), "{options:?} at 128");
        }
    }
    let deepest = resumption(256, MarkedOptions::Terminal);
    assert_eq!(deepest, (769, 2_306), "repeats 256");
}

#[test]
fn nesting_keeps_recorded_documents_below_the_previous_limit() {
    let rows: Vec<Recorded> =
        serde_json::from_str(include_str!("../../tests/fixtures/container_depth.json")).unwrap();
    assert!(!rows.is_empty());
    for row in rows {
        let options = row.options();
        let document = parse_with_options(row.source, options);
        assert_eq!(recorded(&document), row.document, "{}", row.name);
    }
}

#[test]
fn deep_container_documents_drop_on_a_small_stack() {
    on_small_stack(|| {
        drop(nest(
            Node {
                kind: Kind::Gap,
                authored: MappedText::default(),
                range: 0..0,
            },
            100_000,
        ));
    });
}

/// Nest `leaf` under `levels` container nodes, cycling through every container kind.
fn nest(mut node: Node, levels: usize) -> Node {
    for level in 0..levels {
        let kind = match level % 3 {
            0 => Kind::Quote(vec![node], MappedText::default()),
            1 => Kind::Item(vec![node]),
            _ => Kind::List(None, vec![node]),
        };
        node = Node {
            kind,
            authored: MappedText::default(),
            range: 0..0,
        };
    }
    node
}

#[test]
fn deep_containers_index_for_inline_scope_on_a_small_stack() {
    on_small_stack(|| {
        let Document {
            options,
            definitions,
            scopes,
            source,
            nodes,
        } = parse_with_options("deep *text*\n".to_owned(), MarkedOptions::Terminal);
        let range = nodes[0].range.clone();
        let leaf = nest(
            Node {
                kind: Kind::Paragraph(nodes),
                authored: MappedText::default(),
                range: range.clone(),
            },
            100_000,
        );
        let document = Document {
            options,
            definitions,
            scopes,
            source,
            nodes: vec![leaf],
        };
        let context = InlineContext::new(&document);
        assert!(context.scope(&range, false).is_some());
    });
}

/// One recorded document with the caller grammar that produced it.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Recorded {
    /// Case name naming the shape and repeat count.
    name: String,
    /// Grammar selection the caller used.
    policy: Policy,
    /// Authored input.
    source: String,
    /// Recorded node lines.
    document: String,
}

impl Recorded {
    /// Grammar selection this row was recorded under.
    fn options(&self) -> MarkedOptions {
        match self.policy {
            Policy::Terminal => MarkedOptions::Terminal,
            Policy::EscapeHtml => MarkedOptions::Browser { escape_html: true },
            Policy::Browser => MarkedOptions::Browser { escape_html: false },
        }
    }
}

/// Caller grammars the recorded documents cover.
#[derive(serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Policy {
    /// Terminal grammar.
    Terminal,
    /// Browser grammar showing authored tags.
    Browser,
    /// Browser grammar escaping authored tags.
    EscapeHtml,
}

/// An indented list source at `levels` nesting levels.
pub(super) fn indented(levels: usize, prefix: &str) -> String {
    let mut source = format!("{prefix}- a\n");
    for level in 1..levels {
        source.push_str(prefix);
        source.push_str(&"  ".repeat(level));
        source.push_str("- a\n");
    }
    source
}

#[test]
fn deep_list_items_parse_on_a_small_stack() {
    depth_rows(
        WORKER_STACK,
        || format!("{}x", "- ".repeat(1_000)),
        (2_000, 2_002),
    );
}

#[test]
fn deep_indented_list_parses_on_a_small_stack() {
    depth_rows(REDUCED_STACK, || indented(300, ""), (600, 1_200));
}

#[test]
fn deep_reply_list_parses_on_a_small_stack() {
    depth_rows(REDUCED_STACK, || indented(300, "> "), (601, 1_201));
}

/// Work one fresh parse of a depth-`levels` indented chain charges, with the authored
/// size of that chain.
fn item_text_charge(levels: usize, charge: fn(&super::Work) -> usize) -> (usize, usize) {
    let source = indented(levels, "");
    let bytes = source.len();
    super::WORK.with_borrow_mut(|work| *work = super::Work::default());
    let document = parse_with_options(source, MarkedOptions::Terminal);
    assert_eq!(shape(&document).0, 2 * levels);
    (super::WORK.with_borrow(charge), bytes)
}

/// Whether work per authored byte at four times the depth stays within twice the shallower
/// chain's, so an item's work follows its own lines instead of every level above it.
fn stays_proportional([shallow, deep]: [(usize, usize); 2]) -> bool {
    (deep.0 as u128) * (shallow.1 as u128) <= 2 * (shallow.0 as u128) * (deep.1 as u128)
}

#[test]
fn deep_item_text_classification_stays_proportional_to_the_source() {
    let measured = [75, 300].map(|levels| item_text_charge(levels, |work| work.paragraph_bytes));
    assert!(
        stays_proportional(measured),
        "item text classified per authored byte {measured:?}"
    );
}

#[test]
fn deep_item_text_joins_stay_proportional_to_the_source() {
    let measured = [75, 300].map(|levels| item_text_charge(levels, |work| work.joined_bytes));
    assert!(
        stays_proportional(measured),
        "joined item text bytes per authored byte {measured:?}"
    );
}

#[test]
fn deep_item_stop_rules_stay_proportional_to_the_source() {
    let measured =
        [75, 300].map(|levels| item_text_charge(levels, |work| work.block_recognizer_bytes));
    assert!(
        stays_proportional(measured),
        "item stop rule bytes per authored byte {measured:?}"
    );
}

#[test]
fn deep_item_framing_stays_proportional_to_the_source() {
    let measured = [75, 300].map(|levels| item_text_charge(levels, |work| work.framing_bytes));
    assert!(
        stays_proportional(measured),
        "item line framing bytes per authored byte {measured:?}"
    );
}

#[test]
fn deep_item_line_decisions_stay_linear_in_the_line_count() {
    let measured =
        [75, 300].map(|levels| (item_text_charge(levels, |work| work.list_lines).0, levels));
    assert!(
        stays_proportional(measured),
        "item line decisions per source line {measured:?}"
    );
}

/// Work one fresh parse of the repeated resumption message charges, with its authored size.
fn resumption_work(repeats: usize) -> (super::Work, usize) {
    let source = resumption_message(repeats);
    let bytes = source.len();
    super::WORK.with_borrow_mut(|work| *work = super::Work::default());
    let document = parse_with_options(source, MarkedOptions::Terminal);
    assert_eq!(shape(&document).0, 3 * repeats + 1);
    (super::WORK.with_borrow(|work| *work), bytes)
}

#[test]
fn deep_resumed_items_copy_in_proportion_to_the_source() {
    // Every resumed item admits the rest of the message as lazy lines, one level deeper, so
    // a level that copied what the level above it already holds copies the square.
    let copied = [32, 128].map(|repeats| {
        let (work, bytes) = resumption_work(repeats);
        (work.source_bytes + work.joined_bytes, bytes)
    });
    assert!(
        stays_proportional(copied),
        "authored bytes copied per authored byte {copied:?}"
    );
}

/// Line visits of one fresh parser run whose container depth is `depth * levels`.
fn work_of(source: String, depth: usize, levels: usize) -> usize {
    super::WORK.with_borrow_mut(|work| *work = super::Work::default());
    let document = parse_with_options(source, MarkedOptions::Terminal);
    assert_eq!(shape(&document).0, depth * levels);
    super::WORK.with_borrow(|work| work.quote_lines + work.list_lines)
}

#[test]
fn deep_container_line_visits_grow_linearly_with_depth() {
    let quotes = [250, 500, 1_000].map(|depth| work_of(">".repeat(depth), depth, 1));
    assert!(super::grows_linearly(quotes), "quotes {quotes:?}");
    let items =
        [500, 1_000, 2_000].map(|depth| work_of(format!("{}x", "- ".repeat(depth)), depth, 2));
    assert!(super::grows_linearly(items), "items {items:?}");
}

#[test]
fn deep_quote_resolution_completes_on_a_reduced_stack() {
    let measured = on_stack(REDUCED_STACK, || {
        shape(&parse_with_options(
            ">".repeat(2_000),
            MarkedOptions::Terminal,
        ))
    });
    assert_eq!(measured, (2_000, 2_000));
}

/// Work one fresh parse of a single list item holding `paragraphs` blank-separated
/// paragraphs charges, with the authored size of the document.
fn item_paragraph_charge(paragraphs: usize, charge: fn(&super::Work) -> usize) -> (usize, usize) {
    let source = format!("- a\n{}", "\n  a\n".repeat(paragraphs - 1));
    let bytes = source.len();
    super::WORK.with_borrow_mut(|work| *work = super::Work::default());
    let document = parse_with_options(source, MarkedOptions::Terminal);
    assert_eq!(shape(&document), (2, 3 * paragraphs + 1));
    (super::WORK.with_borrow(charge), bytes)
}

#[test]
fn item_stop_seeks_stay_proportional_to_the_item() {
    let measured =
        [50, 200].map(|paragraphs| item_paragraph_charge(paragraphs, |work| work.item_seeks));
    assert!(
        stays_proportional(measured),
        "item stop seeks per authored byte {measured:?}"
    );
}
