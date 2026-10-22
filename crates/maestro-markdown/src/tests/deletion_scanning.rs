//! Deletion opener cost at the shared owner and through both public entries.
use super::{SELECTIONS, WORK, Work, grows_linearly, parsed};
use crate::{Kind, MarkedOptions, Node, tokenizer::inline::delimiters::Delimiters};
use serde::Deserialize;

/// Approximations and unclosed double openers, literal under every caller selection.
const LITERAL_EVERYWHERE: [&str; 2] = ["step takes about ~5 min, ", "~~s "];

/// Strict-grammar workload whose escape unit is refused at every reached opener.
const REFUSED_ESCAPE: &str = "~~a\\\n";

/// Unclosed workloads beside one caller selection that admits their tildes.
fn workloads() -> [(&'static str, MarkedOptions); 3] {
    [
        (
            LITERAL_EVERYWHERE[0],
            MarkedOptions::Browser { escape_html: false },
        ),
        (LITERAL_EVERYWHERE[1], MarkedOptions::Terminal),
        (REFUSED_ESCAPE, MarkedOptions::Terminal),
    ]
}

/// Selections that admit the strict grammar, whose escape unit refuses line terminators.
fn strict_selections() -> [MarkedOptions; 2] {
    [MarkedOptions::Terminal, MarkedOptions::Export]
}

/// Bytes the shared inline owner visits while preparing and answering openers.
fn root_work(piece: &str, options: MarkedOptions) -> [usize; 3] {
    [32, 64, 128].map(|count| {
        let source = crate::source::ScopeSource::identity(&piece.repeat(count));
        let definitions = std::collections::HashMap::new();
        WORK.with_borrow_mut(|work| *work = Work::default());
        let mut tokenizer =
            crate::tokenizer::inline::Tokenizer::new(&source, options, &definitions, None, false);
        assert!(!tokenizer.parse(&[], None).is_empty());
        WORK.with_borrow(|work| work.deletion_bytes)
    })
}

#[test]
fn unclosed_deletion_root_work_is_linear() {
    for (piece, options) in workloads() {
        let counts = root_work(piece, options);
        eprintln!("{options:?} {piece:?} N/2N/4N {counts:?}");
        assert!(counts[0] > 0, "{options:?} {piece:?}");
        assert!(grows_linearly(counts), "{options:?} {piece:?}: {counts:?}");
    }
}

/// Require only text and line breaks, spelled exactly as the source.
///
/// The closing line ending ends the paragraph rather than adding a node.
fn assert_literal(children: &[Node], source: &str, case: &str) {
    assert!(
        children
            .iter()
            .all(|node| matches!(node.kind, Kind::Text(_) | Kind::Break)),
        "{case}"
    );
    assert_eq!(
        children
            .iter()
            .map(|node| node.authored.to_owned_string())
            .collect::<String>(),
        source.trim_end_matches('\n'),
        "{case}"
    );
}

/// Work through one public entry, with the literal result it keeps.
fn entry_work(piece: &str, options: MarkedOptions, extension: bool) -> [usize; 3] {
    [32, 64, 128].map(|count| {
        let source = piece.repeat(count);
        WORK.with_borrow_mut(|state| *state = Work::default());
        let document = parsed(source.clone(), options, extension);
        let Kind::Paragraph(children) = &document.nodes[0].kind else {
            panic!("{options:?} {piece:?}");
        };
        assert_literal(children, &source, &format!("{options:?} {piece:?}"));
        WORK.with_borrow(|state| state.deletion_bytes)
    })
}

/// Assert linear work and unchanged output through both public entries.
fn assert_linear_through_parse(piece: &str, options: MarkedOptions, extension: bool) {
    let counts = entry_work(piece, options, extension);
    eprintln!("{options:?} {piece:?} extension={extension} N/2N/4N {counts:?}");
    assert!(
        grows_linearly(counts),
        "{options:?} {piece:?} extension={extension}: {counts:?}"
    );
    if options == MarkedOptions::Documentation {
        assert_eq!(counts, [0; 3], "{piece:?}");
    }
}

#[test]
fn unclosed_deletion_work_is_linear_through_parse() {
    for piece in LITERAL_EVERYWHERE {
        for options in SELECTIONS {
            assert_linear_through_parse(piece, options, false);
            assert_linear_through_parse(piece, options, true);
        }
    }
    for options in strict_selections() {
        assert_linear_through_parse(REFUSED_ESCAPE, options, false);
        assert_linear_through_parse(REFUSED_ESCAPE, options, true);
    }
}

/// One recorded raw window with its reference-lexer selections per grammar.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Window {
    /// Whole source the window selects from.
    text: String,
    /// Window start in source bytes.
    start: usize,
    /// Window end in source bytes.
    end: usize,
    /// Strict selection: consumed, content start and content end, window-relative bytes.
    strict: Option<[usize; 3]>,
    /// Browser selection: consumed, content start and content end, window-relative bytes.
    browser: Option<[usize; 3]>,
}

/// Observe one selection through the shared owner as the tokenizer reads it.
fn select(text: &str, window: &Window, options: MarkedOptions) -> Option<[usize; 3]> {
    let delimiters = Delimiters::new(text, options);
    let (consumed, body) = delimiters.deletion(text, &(window.start..window.end))?;
    let relative = |offset: usize| offset - window.start;
    Some([consumed, relative(body.start), relative(body.end)])
}

#[test]
fn deletion_lookup_matches_selected_raw_windows() {
    let rows: Vec<Window> =
        serde_json::from_str(include_str!("../../tests/fixtures/deletion_windows.json")).unwrap();
    assert_eq!(rows.len(), 209);
    let mut selections = 0;
    for window in &rows {
        for (options, expected) in [
            (MarkedOptions::Terminal, window.strict),
            (
                MarkedOptions::Browser { escape_html: false },
                window.browser,
            ),
            (MarkedOptions::Browser { escape_html: true }, window.browser),
        ] {
            let actual = select(&window.text, window, options);
            assert_eq!(actual, expected, "{:?} {options:?}", window.text);
            selections += 1;
        }
        assert_eq!(
            select(&window.text, window, MarkedOptions::Documentation),
            None,
            "{:?}",
            window.text
        );
        assert_eq!(
            select(&window.text, window, MarkedOptions::Export),
            window.strict,
            "{:?}",
            window.text
        );
    }
    eprintln!("WINDOWS {} selections {selections}", rows.len());
    assert_eq!(selections, 627);
}
