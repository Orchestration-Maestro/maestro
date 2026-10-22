//! Deep container nesting, its recorded documents and record teardown.
use crate::{Document, Kind, MappedText, MarkedOptions, Node, parse_with_options};

/// Caller grammars every depth row covers.
const OPTIONS: [MarkedOptions; 5] = [
    MarkedOptions::Terminal,
    MarkedOptions::Browser { escape_html: false },
    MarkedOptions::Browser { escape_html: true },
    MarkedOptions::Export,
    MarkedOptions::Documentation,
];

/// Worker stack well below the main thread's, so depth cannot lean on it.
const WORKER_STACK: usize = 2 * 1024 * 1024;

/// Run one case on a worker thread of the witness stack size.
fn on_small_stack<T: Send + 'static>(case: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(WORKER_STACK)
        .spawn(case)
        .expect("worker thread")
        .join()
        .expect("worker panicked")
}

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

/// Children that deepen the container chain, in authored order.
fn children(kind: &Kind) -> Box<dyn Iterator<Item = &Node> + '_> {
    match kind {
        Kind::Paragraph(nodes)
        | Kind::Inline(nodes)
        | Kind::Heading(_, nodes, _)
        | Kind::Quote(nodes, _)
        | Kind::Item(nodes)
        | Kind::List(_, nodes)
        | Kind::Strong(nodes)
        | Kind::Strike(nodes)
        | Kind::Emphasis(nodes)
        | Kind::Link(nodes, _, _, _) => Box::new(nodes.iter()),
        Kind::Image(_, metadata) => Box::new(metadata.children.iter()),
        Kind::Table(table) => Box::new(
            table
                .header
                .iter()
                .chain(table.rows.iter().flatten())
                .flat_map(|cell| cell.children.iter()),
        ),
        _ => Box::new(std::iter::empty()),
    }
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

/// One line per node, indented by depth, naming kind, extent and leaf spelling.
fn recorded(document: &Document) -> String {
    let mut lines = Vec::new();
    let mut pending: Vec<(usize, &Node)> =
        document.nodes.iter().rev().map(|node| (0, node)).collect();
    while let Some((depth, node)) = pending.pop() {
        lines.push(format!(
            "{}{} {}..{}{}",
            "  ".repeat(depth),
            tag(&node.kind),
            node.range.start,
            node.range.end,
            leaf(&node.kind)
        ));
        let kids: Vec<&Node> = children(&node.kind).collect();
        pending.extend(kids.into_iter().rev().map(|child| (depth + 1, child)));
    }
    lines.join("\n")
}

#[test]
fn deep_quote_markers_parse_on_a_small_stack() {
    for options in OPTIONS {
        let measured =
            on_small_stack(move || shape(&parse_with_options(">".repeat(1_000), options)));
        assert_eq!(measured, (1_000, 1_000), "{options:?}");
    }
}

#[test]
fn deep_quote_list_markers_parse_on_a_small_stack() {
    for options in OPTIONS {
        let measured =
            on_small_stack(move || shape(&parse_with_options("> - ".repeat(256), options)));
        assert_eq!(measured, (768, 768), "{options:?}");
    }
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

#[test]
fn repeated_quote_list_resumption_keeps_reference_depth() {
    for (repeats, expected) in [(64, (193, 578)), (128, (385, 1_154)), (256, (769, 2_306))] {
        let case = move || {
            let mut source = String::from("> intro\nlazy\n");
            for _ in 0..repeats {
                source.push_str("> - a\n    cont\n> \n> para\n> \n");
            }
            shape(&parse_with_options(source, MarkedOptions::Terminal))
        };
        assert_eq!(on_small_stack(case), expected, "repeats {repeats}");
    }
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
        let mut node = Node {
            kind: Kind::Gap,
            authored: MappedText::default(),
            range: 0..0,
        };
        for level in 0..100_000 {
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
        drop(node);
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
