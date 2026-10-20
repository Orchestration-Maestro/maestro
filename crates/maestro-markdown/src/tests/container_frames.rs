//! Container lineage and stateful caller observations through complete parsing.
use crate::{BlockToken, BlockTokenizer, Kind, MarkedOptions, Node, parse_with_tokenizer};
use std::cell::RefCell;

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
