//! Captures selected from raw source windows under every caller grammar.
use super::fixture_corpus::Corpus;
use crate::{Kind, MarkedOptions, Node, rules, tokenizer::capture};
use std::ops::Range;

/// Consumed bytes, label offset/length, and the selected reference identity.
type Selection = Option<(usize, usize, usize, String)>;
/// Window endpoints and four caller results.
type Window = (usize, usize, Selection, Selection, Selection, Selection);

/// Expected reference operand kind.
#[derive(Clone, Copy)]
enum Operand {
    /// A link from the authored label and reference.
    Link,
    /// An image from the authored bang and reference.
    Image,
}

/// Positive controls for explicit, collapsed, shortcut, image, emphasis and quote.
const CASES: [(&str, Range<usize>, Operand); 6] = [
    (
        "[a][id]\n\n[a]: /wrong\n[id]: /right \"tip\"",
        0..7,
        Operand::Link,
    ),
    ("[a][]\n\n[a]: /right \"tip\"", 0..5, Operand::Link),
    ("[a]\n\n[a]: /right \"tip\"", 0..3, Operand::Link),
    ("![a][id]\n\n[id]: /right \"tip\"", 0..8, Operand::Image),
    ("*[a][id]*\n\n[id]: /right \"tip\"", 1..8, Operand::Link),
    ("> [a][id]\n\n[id]: /right \"tip\"", 2..9, Operand::Link),
];

#[test]
fn reference_admission_matches_recorded_windows() {
    let corpus: Corpus<Window> =
        serde_json::from_str(include_str!("../../tests/fixtures/reference_windows.json")).unwrap();
    let mut windows = 0;
    let mut selections = [0; 5];
    for source in corpus.texts {
        for (start, end, terminal, browser, documentation, export) in source.windows {
            windows += 1;
            let view = &source.text[start..end];
            for (index, (options, expected)) in [
                (MarkedOptions::Terminal, &terminal),
                (MarkedOptions::Browser { escape_html: false }, &browser),
                (MarkedOptions::Browser { escape_html: true }, &browser),
                (MarkedOptions::Documentation, &documentation),
                (MarkedOptions::Export, &export),
            ]
            .into_iter()
            .enumerate()
            {
                let actual = select(&source.text, start..end, options);
                assert_eq!(&actual, expected, "{options:?} {view:?} {start}..{end}");
                selections[index] += usize::from(expected.is_some());
            }
        }
    }
    assert_eq!(windows, 1404);
    assert_eq!(selections, [380, 404, 404, 404, 380]);
}

/// Run the selected caller's unchanged reference rules.
fn select(source: &str, range: Range<usize>, options: MarkedOptions) -> Selection {
    let text = &source[range.clone()];
    let delimiters = crate::tokenizer::inline::delimiters::Delimiters::new(source, options);
    if !delimiters.reference_possible(text, &range) {
        return None;
    }
    let explicit = rules::inline_reflink(options)?.find(text);
    let matched = explicit
        .clone()
        .or_else(|| rules::inline_nolink(options)?.find(text))?;
    let label = matched.group(1)?;
    let identity = if explicit.is_some() {
        capture(text, &matched, 2)
    } else {
        capture(text, &matched, 1)
    };
    Some((matched.end(), label.start, label.len(), identity.to_owned()))
}

/// Descend through the container wrappers used by these source windows.
fn first_operand(nodes: &[Node]) -> &Node {
    let node = nodes.first().unwrap();
    match &node.kind {
        Kind::Paragraph(children) | Kind::Emphasis(children) | Kind::Quote(children, _) => {
            first_operand(children)
        }
        _ => node,
    }
}

/// Resolved operands of a reference link or image.
fn operands(node: &Node) -> (&str, &str, &str, bool) {
    match &node.kind {
        Kind::Link(_, label, href, metadata) => (label, href, &metadata.title, false),
        Kind::Image(label, metadata) => (label, &metadata.destination, &metadata.title, true),
        _ => panic!("reference operand"),
    }
}

/// One positive control keeps its authored extent and resolved metadata.
fn resolved(
    source: &str,
    range: Range<usize>,
    operand: Operand,
    options: MarkedOptions,
    route: bool,
) {
    let document = super::parsed(source.into(), options, route);
    let node = first_operand(&document.nodes);
    assert_eq!(node.range, range, "{source:?} {options:?} route={route}");
    assert_eq!(node.authored.to_owned_string(), &source[range]);
    let (label, href, title, actual_image) = operands(node);
    assert_eq!(
        actual_image,
        matches!(operand, Operand::Image),
        "{source:?} {options:?} route={route}"
    );
    assert_eq!((label, href, title), ("a", "/right", "tip"));
}

#[test]
fn reference_results_keep_definition_precedence_and_ranges() {
    for options in super::SELECTIONS {
        for route in [false, true] {
            let missing = super::parsed("[a][id]\n\n[a]: /wrong".into(), options, route);
            let Kind::Paragraph(children) = &missing.nodes[0].kind else {
                panic!("paragraph");
            };
            super::assert_literal(children, "[a][id]", "missing explicit definition");
            for (source, range, operand) in CASES {
                resolved(source, range, operand, options, route);
            }
        }
    }
}
