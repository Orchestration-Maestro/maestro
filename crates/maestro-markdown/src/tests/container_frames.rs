//! Container lineage and stateful caller observations through complete parsing.
use crate::{BlockToken, BlockTokenizer, Kind, MarkedOptions, Node, parse_with_tokenizer};
use std::{cell::RefCell, fmt::Write};

#[test]
fn reply_quote_reentry_keeps_ancestor_body_lineage() {
    let document = crate::parse("> a\n> > b\n> > > c\n> a\n> > b\n> > > c".to_owned());
    let Kind::Quote(outer, _) = &document.nodes[0].kind else {
        panic!("outer quote")
    };
    let parent = &outer[1];
    let Kind::Quote(children, body) = &parent.kind else {
        panic!("parent quote")
    };
    assert_eq!(parent.range, 6..35);
    assert_eq!(body.to_owned_string(), "b\n> c\na\nb\n> c");
    assert_eq!(children[1].range, 14..35);
    assert_eq!(children[1].authored.to_owned_string(), "> c\na\n> b\n> > c");
}

#[test]
fn container_definition_lookahead_keeps_later_title_lines() {
    let options = [
        MarkedOptions::Terminal,
        MarkedOptions::Documentation,
        MarkedOptions::Browser { escape_html: false },
        MarkedOptions::Browser { escape_html: true },
        MarkedOptions::Export,
    ];
    for ((prefix, source), options) in [
        ("", "[a]: /x\n\"title\ncontinued\"\n\n[a]"),
        ("> ", "> [a]: /x\n> \"title\n> continued\"\n>\n> [a]"),
        ("  ", "- [a]: /x\n  \"title\n  continued\"\n\n  [a]"),
        (
            ">   ",
            "> - [a]: /x\n>   \"title\n>   continued\"\n>\n>   [a]",
        ),
    ]
    .into_iter()
    .flat_map(|case| options.map(|options| (case, options)))
    {
        let end = source.find(&format!("{prefix}continued")).unwrap();
        let partial = crate::parse_with_options(source[..end].to_owned(), options);
        let complete = crate::parse_with_options(source.to_owned(), options);
        assert_eq!(partial.definitions["a"].title, "");
        assert_eq!(complete.definitions["a"].title, "title\ncontinued");
        let mut node = &complete.nodes[0];
        loop {
            match &node.kind {
                Kind::Link(_, _, destination, metadata) => {
                    assert_eq!(destination, "/x");
                    assert_eq!(metadata.title, "title\ncontinued");
                    break;
                }
                Kind::Quote(children, _)
                | Kind::List(_, children)
                | Kind::Item(children)
                | Kind::Paragraph(children)
                | Kind::Inline(children) => node = &children[0],
                _ => panic!("reference link missing"),
            }
        }
    }
    let prefix = "[a]: /x\n\"title\n";
    let full = "[a]: /x\n\"title\ncontinued\"\n\n[a]";
    let partial = crate::rules::definition::recognize(prefix, MarkedOptions::Terminal).unwrap();
    let complete = crate::rules::definition::recognize(full, MarkedOptions::Terminal).unwrap();
    assert!(partial.consumed < prefix.len());
    assert_ne!(partial.consumed, complete.consumed);
    for source in ["[a]: /x\nplain\n", "[a]: /x\nplain\ncontinued\n\n[a]"] {
        assert_eq!(crate::parse(source.to_owned()).definitions["a"].title, "");
    }
}

#[test]
fn definition_chunks_preserve_grammar_alternative_priority() {
    use crate::rules::definition::{Recognizer, Verdict};
    for (source, destination, title) in [
        ("[a]: <x> \"unfinished >", "<x> \"unfinished >", None),
        ("[a]: /x \"a\\\"\nlater\"", "/x", Some("\"a\\\"\nlater\"")),
        ("[a]: /x 'first\nlast'", "/x", Some("'first\nlast'")),
        ("[a]: /x (first\nlast)", "/x", Some("(first\nlast)")),
    ] {
        let mut parser = Recognizer::new(MarkedOptions::Terminal);
        super::WORK.with_borrow_mut(|work| work.definition_bytes = 0);
        for (offset, scalar) in source.char_indices() {
            assert!(matches!(
                parser.extend(&source[offset..offset + scalar.len_utf8()]),
                Verdict::NeedMore
            ));
        }
        let Some(matched) = parser.finish() else {
            panic!("complete definition")
        };
        assert_eq!(&source[matched.destination], destination);
        assert_eq!(matched.title.map(|range| &source[range]), title);
        assert_eq!(matched.consumed, source.len());
        assert_eq!(
            super::WORK.with_borrow(|work| work.definition_bytes),
            source.len()
        );
        assert!(matches!(parser.extend("ignored"), Verdict::Stable(Some(_))));
        assert_eq!(
            super::WORK.with_borrow(|work| work.definition_bytes),
            source.len()
        );
    }
}

#[test]
fn definition_chunks_wait_for_optional_title_and_terminators() {
    use crate::rules::definition::{Recognizer, Verdict};
    let mut parser = Recognizer::new(MarkedOptions::Terminal);
    assert!(matches!(
        parser.extend("[a]: /x\n\"title\n"),
        Verdict::NeedMore
    ));
    assert!(matches!(parser.extend("continued\"\n"), Verdict::NeedMore));
    let Verdict::Stable(Some(matched)) = parser.extend("next") else {
        panic!("closed title")
    };
    assert_eq!(matched.consumed, "[a]: /x\n\"title\ncontinued\"\n".len());
}

#[test]
fn long_definition_titles_advance_without_rescanning() {
    for count in [16, 32, 64] {
        let mut source = String::new();
        for index in 0..count {
            write!(source, "[r{index}]: /x\n\"").unwrap();
            source.push_str(&"title line\n".repeat(64));
            source.push_str("closed\"\n\n");
        }
        let bytes = source.len();
        super::WORK.with_borrow_mut(|work| *work = super::Work::default());
        let document = crate::parse(source);
        assert_eq!(document.definitions.len(), count);
        let work = super::WORK.with_borrow(|work| work.definition_bytes);
        eprintln!("definitions={count}, source_bytes={bytes}, definition_bytes={work}");
        assert!(
            work <= bytes + count,
            "definition bytes {work} for {bytes} source bytes"
        );
    }
}

/// First leaf's observable kind and operand after removing container wrappers.
fn first_leaf(nodes: &[Node]) -> (&'static str, String) {
    let mut node = &nodes[0];
    loop {
        node = match &node.kind {
            Kind::Quote(children, _) | Kind::List(_, children) | Kind::Item(children) => {
                &children[0]
            }
            Kind::Heading(_, _, label) => return ("heading", label.clone()),
            Kind::CodeBlock(body, _) => return ("code", body.trim_end_matches('\n').to_owned()),
            Kind::HtmlBlock(body) => return ("html", body.trim_end_matches('\n').to_owned()),
            Kind::Table(table) => return ("table", table.authored.clone()),
            Kind::Paragraph(_) | Kind::Inline(_) => {
                return (
                    "text",
                    node.authored
                        .to_owned_string()
                        .trim_end_matches('\n')
                        .to_owned(),
                );
            }
            _ => panic!("expected a multiline leaf"),
        };
    }
}

/// Complete and clipped operands, with their full leaf kind and payload.
const LEAF_CASES: [(&str, &str, &str, &str); 7] = [
    ("alpha\nbeta\n===", "alpha\nbeta", "heading", "alpha\nbeta"),
    (
        "a | b\n---|---\nx | y",
        "a | b",
        "table",
        "a | b\n---|---\nx | y",
    ),
    (
        "<!--\nalpha\nbeta\n-->",
        "<!--\nalpha",
        "html",
        "<!--\nalpha\nbeta\n-->",
    ),
    (
        "<script>\nalpha\nbeta\n</script>",
        "<script>\nalpha",
        "html",
        "<script>\nalpha\nbeta\n</script>",
    ),
    (
        "~~~ rust\nalpha\nbeta\n~~~",
        "~~~ rust\nalpha",
        "code",
        "alpha\nbeta",
    ),
    ("    alpha\n    beta", "    alpha", "code", "alpha\nbeta"),
    ("alpha\nbeta", "alpha", "text", "alpha\nbeta"),
];

#[test]
fn container_multiline_leaves_keep_later_input() {
    let options = [
        MarkedOptions::Terminal,
        MarkedOptions::Documentation,
        MarkedOptions::Browser { escape_html: false },
        MarkedOptions::Browser { escape_html: true },
        MarkedOptions::Export,
    ];
    for ((full, partial, kind, operand), (position, options)) in
        LEAF_CASES.into_iter().flat_map(|case| {
            (0..4).flat_map(move |position| options.map(|options| (case, (position, options))))
        })
    {
        let complete = crate::parse_with_options(wrap_leaf(full, position), options);
        let prefix = crate::parse_with_options(wrap_leaf(partial, position), options);
        let literal_html = kind == "html"
            && (options == MarkedOptions::Export
                || options == (MarkedOptions::Browser { escape_html: true })
                    && full.starts_with("<script>"));
        let literal_table = kind == "table" && options == MarkedOptions::Documentation;
        let expected_kind = if literal_html || literal_table {
            "text"
        } else {
            kind
        };
        assert_eq!(
            first_leaf(&complete.nodes),
            (expected_kind, operand.to_owned()),
            "{position} {options:?}"
        );
        assert_ne!(
            first_leaf(&complete.nodes),
            first_leaf(&prefix.nodes),
            "{position} {options:?}"
        );
    }
}

#[test]
fn container_paragraph_lookahead_keeps_later_setext() {
    let prefix = "alpha\nbeta\n";
    let full = "alpha\nbeta\n===\n";
    for (position, options) in (0..4).flat_map(|position| {
        [
            MarkedOptions::Terminal,
            MarkedOptions::Documentation,
            MarkedOptions::Browser { escape_html: false },
            MarkedOptions::Browser { escape_html: true },
            MarkedOptions::Export,
        ]
        .map(|options| (position, options))
    }) {
        let partial = crate::parse_with_options(wrap_leaf(prefix, position), options);
        let complete = crate::parse_with_options(wrap_leaf(full, position), options);
        let control =
            crate::parse_with_options(wrap_leaf("alpha\nbeta\n\n===\n", position), options);
        assert_eq!(
            first_leaf(&partial.nodes),
            ("text", "alpha\nbeta".to_owned())
        );
        assert_eq!(
            first_leaf(&complete.nodes),
            ("heading", "alpha\nbeta".to_owned())
        );
        assert_eq!(
            first_leaf(&control.nodes),
            ("text", "alpha\nbeta".to_owned())
        );
        let matched = crate::rules::block_paragraph(options)
            .unwrap()
            .find(prefix)
            .unwrap();
        assert!(matched.end() < prefix.len());
        assert!(
            crate::rules::block_lheading(options)
                .unwrap()
                .find(prefix)
                .is_none()
        );
        assert!(
            crate::rules::block_lheading(options)
                .unwrap()
                .find(full)
                .is_some()
        );
    }
}

/// Author one identical local operand in each independently admitted position.
fn wrap_leaf(source: &str, position: usize) -> String {
    let lines: Vec<_> = source.split('\n').collect();
    match position {
        0 => source.to_owned(),
        1 => lines
            .iter()
            .map(|line| format!("> {line}"))
            .collect::<Vec<_>>()
            .join("\n"),
        2 => format!("- {}", lines.join("\n  ")),
        3 => format!("> - {}", lines.join("\n>   ")),
        _ => unreachable!(),
    }
}

/// An ordered callback transcript with distinguishable repeated admissions.
#[derive(Default)]
struct Observed {
    /// Callback kind and exact operand, including repeated occurrences.
    events: RefCell<Vec<(&'static str, String)>>,
    /// Monotonically increasing payload identity assigned on admission.
    admitted: usize,
}

impl BlockTokenizer for Observed {
    fn start(&self, source: &str) -> Option<usize> {
        self.events.borrow_mut().push(("start", source.to_owned()));
        source.find("!x")
    }

    fn tokenize(&mut self, source: &str) -> Option<BlockToken> {
        self.events.get_mut().push(("tokenize", source.to_owned()));
        source.strip_prefix("!x")?;
        self.admitted += 1;
        Some(BlockToken {
            raw: if source.starts_with("!x\n") {
                "!x\n"
            } else {
                "!x"
            }
            .to_owned(),
            kind: "observed".to_owned(),
            text: self.admitted.to_string(),
        })
    }
}

/// Retained callback payloads in semantic document order, without deduplication.
fn payloads(nodes: &[Node]) -> Vec<&str> {
    let mut pending: Vec<_> = nodes.iter().rev().collect();
    let mut values = Vec::new();
    while let Some(node) = pending.pop() {
        match &node.kind {
            Kind::CustomBlock(_, text) => values.push(text.as_str()),
            Kind::Quote(children, _) | Kind::List(_, children) | Kind::Item(children) => {
                pending.extend(children.iter().rev());
            }
            _ => {}
        }
    }
    values
}

#[test]
fn caller_start_observes_empty_and_item_operands() {
    for (source, expected) in [("é", ""), ("- é", ""), ("- abc", "bc"), ("abc", "bc")] {
        let mut observer = Observed::default();
        let document = parse_with_tokenizer(
            source.to_owned(),
            MarkedOptions::Terminal,
            Some(&mut observer),
        )
        .unwrap();
        assert!(!document.nodes.is_empty());
        let starts = observer
            .events
            .into_inner()
            .into_iter()
            .filter(|(kind, _)| *kind == "start")
            .map(|(_, text)| text)
            .collect::<Vec<_>>();
        assert_eq!(starts, [expected], "{source}");
    }
}

/// Input, ordered callback transcript, total admissions and retained payload identities.
type CallbackCase = (
    &'static str,
    &'static [(&'static str, &'static str)],
    usize,
    &'static [&'static str],
);

/// Independently recorded caller observations for quote/list reentry and plain controls.
const CALLBACK_CASES: &[CallbackCase] = &[
    ("", &[], 0, &[]),
    (
        "> !x\n> > !x\nlazy\n> tail",
        &[
            ("tokenize", "> !x\n> > !x\nlazy\n> tail"),
            ("tokenize", "!x\n> !x"),
            ("tokenize", "> !x"),
            ("tokenize", "!x"),
            ("tokenize", "!x"),
            ("tokenize", "lazy\ntail"),
            ("start", "azy\ntail"),
        ],
        3,
        &["1", "3"],
    ),
    (
        "> - !x\n      p\n> [s]: /y\ne",
        &[
            ("tokenize", "> - !x\n      p\n> [s]: /y\ne"),
            ("tokenize", "- !x"),
            ("tokenize", "!x"),
            ("tokenize", "!x\n    p"),
            ("tokenize", "    p"),
            ("tokenize", "\n[s]: /y"),
            ("tokenize", "[s]: /y"),
            ("tokenize", "!x\n    p"),
            ("tokenize", "    p"),
            ("tokenize", "\n\ne"),
            ("tokenize", "e"),
            ("start", ""),
        ],
        3,
        &["3"],
    ),
    (
        "!x\n\nplain",
        &[
            ("tokenize", "!x\n\nplain"),
            ("tokenize", "\nplain"),
            ("tokenize", "plain"),
            ("start", "lain"),
        ],
        1,
        &["1"],
    ),
];

#[test]
fn container_resume_preserves_caller_tokenizer_observations() {
    for &(source, expected, admissions, retained) in CALLBACK_CASES {
        let mut observer = Observed::default();
        let document = parse_with_tokenizer(
            source.to_owned(),
            MarkedOptions::Terminal,
            Some(&mut observer),
        )
        .unwrap();
        let transcript = observer.events.borrow();
        let transcript: Vec<_> = transcript
            .iter()
            .map(|(kind, text)| (*kind, text.as_str()))
            .collect();
        assert_eq!(transcript, expected, "callback transcript");
        assert_eq!(observer.admitted, admissions);
        assert_eq!(payloads(&document.nodes), retained);
    }
}

/// Reply quoting with a fixed cycle and no final separator.
fn reply_source(lines: usize, cycle: usize) -> String {
    (0..lines)
        .map(|index| "> ".repeat(index % cycle + 1) + "message")
        .collect::<Vec<_>>()
        .join("\n")
}

/// Observe complete parser work without installing a caller callback.
fn measured_reply(lines: usize, cycle: usize) -> super::Work {
    super::WORK.with_borrow_mut(|work| *work = super::Work::default());
    let document = crate::parse(reply_source(lines, cycle));
    observe_reply(&document, lines);
    super::WORK.with_borrow(|work| *work)
}

/// Keep parsed message content observable beside cost and storage measurements.
fn observe_reply(document: &crate::Document, lines: usize) {
    let mut pending: Vec<_> = document.nodes.iter().collect();
    let mut words = 0;
    while let Some(node) = pending.pop() {
        match &node.kind {
            Kind::Quote(children, _) | Kind::Paragraph(children) => pending.extend(children),
            Kind::Text(text) => words += text.decoded.matches("message").count(),
            _ => panic!("unexpected reply block"),
        }
    }
    assert_eq!(words, lines);
}

/// Charge recognizer operands, line framing, source copies and source projections.
fn container_work(work: super::Work) -> usize {
    work.block_recognizer_bytes
        + work.framing_bytes
        + work.source_bytes
        + work.joined_bytes
        + work.source_views
        + work.projections
}

#[test]
fn reply_quote_mapping_grows_linearly_root_witness() {
    let discriminator = [64, 128].map(|lines| measured_reply(lines, 3));
    let source_work =
        discriminator.map(|work| work.source_bytes + work.joined_bytes + work.quote_lines);
    eprintln!("64/128 source work={source_work:?} samples={discriminator:?}");
    assert!(
        source_work[1] <= 3 * source_work[0],
        "repeated raw tails: {source_work:?}"
    );
    let mut failures = Vec::new();
    for (cycle, sizes) in [
        (3, [64, 128, 256]),
        (3, [120, 240, 480]),
        (40, [120, 240, 480]),
    ] {
        let samples = sizes.map(|lines| measured_reply(lines, cycle));
        let charged = samples.map(container_work);
        eprintln!("cycle={cycle} sizes={sizes:?} charged={charged:?} samples={samples:?}");
        for pair in charged.windows(2) {
            if pair[1] > 3 * pair[0] {
                failures.push((cycle, sizes, charged));
                break;
            }
        }
    }
    assert!(
        failures.is_empty(),
        "repeated container operands: {failures:?}"
    );
}

/// Account for all capacities reachable in the reply document, not overlapping view lengths.
fn retained_reply(lines: usize, cycle: usize) -> super::Retained {
    use std::mem::size_of;
    let document = crate::parse(reply_source(lines, cycle));
    observe_reply(&document, lines);
    assert!(document.definitions.is_empty());
    let mut retained = super::Retained::default();
    retained.bytes += document.source.capacity()
        + document.scopes.capacity()
            * size_of::<(crate::inline::ScopeKey, crate::source::ScopeSource)>();
    for scope in document.scopes.values() {
        scope.retained(&mut retained);
    }
    let mut pending = vec![&document.nodes];
    while let Some(nodes) = pending.pop() {
        retained.bytes += nodes.capacity() * size_of::<Node>();
        for node in nodes {
            node.authored.retained(&mut retained);
            match &node.kind {
                Kind::Quote(children, body) => {
                    body.retained(&mut retained);
                    pending.push(children);
                }
                Kind::Paragraph(children) => pending.push(children),
                Kind::Text(text) => {
                    retained.bytes += text.authored.capacity() + text.decoded.capacity();
                }
                _ => panic!("unexpected reply payload"),
            }
        }
    }
    retained
}

#[test]
fn reply_quote_retained_bytes_grow_linearly_symptom_witness() {
    let discriminator = [128, 256].map(|lines| retained_reply(lines, 3));
    let source = discriminator.each_ref().map(|sample| sample.source_bytes);
    let total = discriminator.each_ref().map(|sample| sample.bytes);
    eprintln!("128/256 unique source={source:?} total retained payload={total:?}");
    assert!(source[1] <= 3 * source[0], "retained raw tails: {source:?}");
    for cycle in [3, 40] {
        let bytes = [120, 240, 480].map(|lines| retained_reply(lines, cycle).bytes);
        eprintln!("cycle={cycle} retained payload={bytes:?}");
        assert!(
            bytes.windows(2).all(|pair| pair[1] <= 3 * pair[0]),
            "{bytes:?}"
        );
    }
}

#[test]
fn uniform_quote_control_grows_linearly() {
    let samples = [120, 240, 480].map(|lines| measured_reply(lines, 1));
    let work = samples.map(container_work);
    let bytes = [120, 240, 480].map(|lines| retained_reply(lines, 1).bytes);
    eprintln!("uniform work={work:?} retained payload={bytes:?} samples={samples:?}");
    assert!(
        work.windows(2).all(|pair| pair[1] <= 3 * pair[0]),
        "{work:?}"
    );
    assert!(
        bytes.windows(2).all(|pair| pair[1] <= 3 * pair[0]),
        "{bytes:?}"
    );
}

#[test]
fn callback_operand_accounting_keeps_requested_output_separate() {
    for (list, n, expected) in [
        (true, 16, (1, 0, 71, 2657, 18)),
        (true, 32, (1, 0, 135, 8881, 34)),
        (true, 64, (1, 0, 263, 32081, 66)),
        (true, 128, (1, 0, 519, 121_489, 130)),
        (false, 16, (17, 334, 30, 934, 0)),
        (false, 32, (32, 682, 55, 1904, 0)),
        (false, 64, (65, 1390, 110, 3878, 0)),
        (false, 128, (128, 2794, 215, 7792, 0)),
    ] {
        let source = if list {
            "> - !x\n".to_owned() + &"      p\n> [s]: /y\n".repeat(n) + "e"
        } else {
            reply_source(n, 3)
        };
        let mut observer = Observed::default();
        let document =
            parse_with_tokenizer(source, MarkedOptions::Terminal, Some(&mut observer)).unwrap();
        assert!(!document.nodes.is_empty());
        let mut counts = (0, 0, 0, 0, observer.admitted);
        for (kind, operand) in observer.events.into_inner() {
            if kind == "start" {
                counts.0 += 1;
                counts.1 += operand.len();
            } else {
                counts.2 += 1;
                counts.3 += operand.len();
            }
        }
        assert_eq!(counts, expected, "list={list} n={n}");
    }
}
