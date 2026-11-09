//! Deep inline span nesting parses, and its record is released, without native recursion.
use super::{REDUCED_STACK, WORKER_STACK, nodes::children, on_stack};
use crate::{Kind, MarkedOptions, Node, parse_with_options};

/// Caller grammars every inline depth row covers.
const OPTIONS: [MarkedOptions; 4] = [
    MarkedOptions::Terminal,
    MarkedOptions::Browser { escape_html: false },
    MarkedOptions::Export,
    MarkedOptions::Documentation,
];

/// Short name of one formatted span kind, or the leaf kinds a chain ends in.
fn tag(kind: &Kind) -> &'static str {
    match kind {
        Kind::Paragraph(_) => "paragraph",
        Kind::Strong(_) => "strong",
        Kind::Emphasis(_) => "emphasis",
        Kind::Strike(_) => "strike",
        Kind::Link(..) => "link",
        Kind::Text(_) => "text",
        _ => "other",
    }
}

/// Kinds of the single chain a document nests in, outermost first, as run lengths.
fn chain(nodes: &[Node]) -> Vec<(&'static str, usize)> {
    let mut runs: Vec<(&'static str, usize)> = Vec::new();
    let mut level: Vec<&Node> = nodes.iter().collect();
    while let [node] = level[..] {
        match runs.last_mut() {
            Some((name, count)) if *name == tag(&node.kind) => *count += 1,
            _ => runs.push((tag(&node.kind), 1)),
        }
        level = children(&node.kind).collect();
    }
    runs
}

/// Parse `source` on a worker of `stack` bytes and report the chain it nests in.
fn parsed_chain(
    stack: usize,
    source: String,
    options: MarkedOptions,
) -> Vec<(&'static str, usize)> {
    on_stack(stack, move || {
        chain(&parse_with_options(source, options).nodes)
    })
}

#[test]
fn deep_strong_runs_parse_on_a_small_stack() {
    let depth = 2_600;
    let source = format!("{}x{}", "**".repeat(depth), "**".repeat(depth));
    let chain = parsed_chain(WORKER_STACK, source, MarkedOptions::Terminal);
    assert_eq!(chain, [("paragraph", 1), ("strong", depth), ("text", 1)]);
}

#[test]
fn deep_emphasis_runs_nest_for_every_grammar() {
    let depth = 600;
    for options in OPTIONS {
        for marker in ["*", "_"] {
            let run = marker.repeat(2 * depth + 1);
            let chain = parsed_chain(REDUCED_STACK, format!("{run}x{run}"), options);
            let expected = [
                ("paragraph", 1),
                ("emphasis", 1),
                ("strong", depth),
                ("text", 1),
            ];
            assert_eq!(chain, expected, "{options:?} {marker}");
        }
    }
}

#[test]
fn deep_strong_runs_inside_strike_parse_on_a_small_stack() {
    let depth = 600;
    for options in &OPTIONS[..3] {
        let source = format!("~~{}x{}~~", "**".repeat(depth), "**".repeat(depth));
        let chain = parsed_chain(REDUCED_STACK, source, *options);
        let expected = [
            ("paragraph", 1),
            ("strike", 1),
            ("strong", depth),
            ("text", 1),
        ];
        assert_eq!(chain, expected, "{options:?}");
    }
}

#[test]
fn deep_strong_runs_inside_link_labels_parse_on_a_small_stack() {
    let depth = 1_500;
    for options in &OPTIONS[..2] {
        let source = format!("[{}x{}](u)", "**".repeat(depth), "**".repeat(depth));
        let chain = parsed_chain(REDUCED_STACK, source, *options);
        let expected = [
            ("paragraph", 1),
            ("link", 1),
            ("strong", depth),
            ("text", 1),
        ];
        assert_eq!(chain, expected, "{options:?}");
    }
}
