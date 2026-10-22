//! Recorded code-span selections through the shared inline tokenizer.
use super::SELECTIONS;
use crate::{Kind, source::ScopeSource};
use serde::Deserialize;
use std::collections::HashMap;

/// Recorded sources and their windows.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Whole sources the windows select from, each with its own windows.
    texts: Vec<Source>,
}

/// One whole source with every recorded window over it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Source {
    /// Whole source text.
    text: String,
    /// Window start, window end, then the recorded code-span selection.
    windows: Vec<Window>,
}

/// Window start, window end and the selected consumed bytes and content.
type Window = (usize, usize, Option<Selection>);

/// Consumed bytes and normalized content of one selected span.
type Selection = (usize, String);

#[test]
fn code_span_lookup_matches_selected_raw_windows() {
    let corpus: Corpus =
        serde_json::from_str(include_str!("../../tests/fixtures/code_span_windows.json")).unwrap();
    let mut windows = 0;
    let mut selected = 0;
    for source in &corpus.texts {
        for (start, end, expected) in &source.windows {
            windows += 1;
            let text = &source.text[*start..*end];
            for options in SELECTIONS {
                let actual = select(text, options);
                assert_eq!(&actual, expected, "{text:?} {options:?}");
                selected += usize::from(expected.is_some());
            }
        }
    }
    assert_eq!(windows, 863);
    assert_eq!(selected, 151 * 5);
}

/// The code span a window produces through the configured grammar.
fn select(text: &str, options: crate::MarkedOptions) -> Option<(usize, String)> {
    let source = ScopeSource::identity(text);
    let definitions = HashMap::new();
    let mut tokenizer =
        crate::tokenizer::inline::Tokenizer::new(&source, options, &definitions, None, false);
    let nodes = tokenizer.parse(&[], None);
    match nodes.first().map(|node| (&node.range, &node.kind)) {
        Some((range, Kind::Code(content))) => Some((range.end, content.clone())),
        _ => None,
    }
}
