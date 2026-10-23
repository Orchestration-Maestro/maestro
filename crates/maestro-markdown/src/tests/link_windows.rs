//! Recorded link admission at the shared owner and the pinned rule.
use super::fixture_corpus::Corpus;
use crate::{
    MarkedOptions, rules,
    tokenizer::{capture, inline::delimiters::Delimiters},
};

/// Window start, window end and the four recorded caller selections.
type Window = (usize, usize, Selection, Selection, Selection, Selection);

/// Consumed bytes, label extent, destination and tooltip of one selection.
type Selection = Option<(usize, usize, usize, String, String)>;

#[test]
fn link_admission_matches_selected_raw_windows() {
    let corpus: Corpus<Window> =
        serde_json::from_str(include_str!("../../tests/fixtures/link_windows.json")).unwrap();
    let mut windows = 0;
    let mut selected = 0;
    for source in &corpus.texts {
        for (start, end, terminal, browser, documentation, export) in &source.windows {
            windows += 1;
            let text = &source.text[*start..*end];
            let expected = [
                (MarkedOptions::Terminal, terminal),
                (MarkedOptions::Browser { escape_html: false }, browser),
                (MarkedOptions::Browser { escape_html: true }, browser),
                (MarkedOptions::Documentation, documentation),
                (MarkedOptions::Export, export),
            ];
            for (options, expected) in expected {
                let actual = select(text, options);
                assert_eq!(&actual, expected, "{text:?} {options:?}");
                selected += usize::from(expected.is_some());
            }
        }
    }
    assert_eq!(windows, 1048);
    assert_eq!(selected, 92 + 96 * 3 + 90);
}

/// Select one window through the shared owner and the pinned caller rule.
fn select(text: &str, options: MarkedOptions) -> Selection {
    let delimiters = Delimiters::new(text, options);
    let matched = rules::inline_link(options)?.find(text)?;
    let label = matched.group(1)?;
    let closer = delimiters.link_closer(text, &(0..text.len()))?;
    let start = 1 + usize::from(text.starts_with('!'));
    assert_eq!(closer, start + label.len(), "{text:?}");
    Some((
        matched.end(),
        start,
        label.len(),
        capture(text, &matched, 2).to_owned(),
        capture(text, &matched, 3).to_owned(),
    ))
}
