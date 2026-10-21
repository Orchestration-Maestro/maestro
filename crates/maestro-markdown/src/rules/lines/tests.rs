//! Recognizer equivalence on shared-source windows across UTF-8 chunk boundaries.
use super::{Cursor, Input};
use crate::{
    MarkedOptions, rules,
    source::{ScopeSource, Segmented},
};

/// Supply every scalar as its own shared selection, retaining recognizer progress.
macro_rules! compare {
    ($constructor:expr, $input:expr) => {{
        let input = $input;
        let backing = ScopeSource::identity(input);
        let mut source = Segmented::from(ScopeSource::identity(""));
        let mut actual = $constructor;
        for (start, scalar) in input.char_indices() {
            source.append(
                &backing
                    .select(std::iter::once(start..start + scalar.len_utf8()))
                    .into(),
            );
            actual.extend(&source);
        }
        let observed = actual.finish(&source);
        let expected = ($constructor).finish(input);
        assert_eq!(observed, expected, "{input:?}");
        observed
    }};
}

#[test]
fn paragraph_windows_keep_multiline_lookahead_and_unicode() {
    for options in [
        MarkedOptions::Terminal,
        MarkedOptions::Documentation,
        MarkedOptions::Browser { escape_html: true },
        MarkedOptions::Export,
    ] {
        let heading = compare!(
            rules::paragraph::Recognizer::new(options),
            "é😀\nsecond\n---\n\nnext"
        );
        assert_eq!(heading.heading.unwrap().content, 0..13);
        compare!(
            rules::paragraph::Recognizer::new(options),
            "é😀\nhead | tail\n--- | ---\nvalue\n"
        );
    }
}

#[test]
fn code_windows_keep_indentation_blank_lines_and_final_scalars() {
    let code = compare!(
        rules::code::Recognizer::default(),
        "    é😀\n\tsecond\n\nnext"
    );
    assert_eq!(code, Some(20));
    compare!(rules::code::Recognizer::default(), "    😀");
}

#[test]
fn fence_windows_keep_the_opening_delimiter_and_unterminated_body() {
    let fence = compare!(
        rules::fence::Recognizer::default(),
        "  ```rust\n  é😀\n  ```\ntail"
    )
    .unwrap();
    assert_eq!(fence.indent, 2);
    assert_eq!(fence.info, 5..9);
    for input in ["```\n😀", "```\n😀\n", "~~~\n😀\n~~~~\n"] {
        compare!(rules::fence::Recognizer::default(), input);
    }
}

#[test]
fn table_windows_keep_split_header_delimiter_and_row_boundaries() {
    for options in [
        MarkedOptions::Terminal,
        MarkedOptions::Browser { escape_html: true },
        MarkedOptions::Export,
    ] {
        let table = compare!(
            rules::table::Recognizer::new(options),
            "é | 😀\n--- | ---\na | b\n\n# next"
        )
        .unwrap();
        assert_eq!(table.header, 0..9);
        compare!(rules::table::Recognizer::new(options), "é | 😀\n--- | ---");
    }
}

#[test]
fn html_windows_keep_selected_terminators_and_multiline_closing_tags() {
    for input in [
        "<!-- é😀\nend -->\n\ntail",
        "<script>\né😀\n</script>\nnext",
        "</custom\n  >\né😀\n\nnext",
    ] {
        assert!(compare!(rules::html::Recognizer::default(), input).is_some());
    }
}

#[test]
fn shared_windows_borrow_contiguous_operands_and_charge_only_split_bytes() {
    let first = ScopeSource::identity("é😀\n");
    let mut source = Segmented::from(first);
    source.append(&ScopeSource::identity("next\n").into());
    assert!(matches!(
        source.window(0..6),
        std::borrow::Cow::Borrowed("é😀")
    ));
    crate::tests::WORK.with_borrow_mut(|work| *work = crate::tests::Work::default());
    assert_eq!(source.window(2..9), "😀\nne");
    let mut cursor = Cursor::default();
    assert_eq!(cursor.next(&source), Some(0..7));
    assert_eq!(cursor.next(&source), Some(7..12));
    assert_eq!(cursor.next(&source), None);
    crate::tests::WORK.with_borrow(|work| {
        assert_eq!(work.framing_bytes, 12);
        assert_eq!(work.joined_bytes, 7);
    });
}

#[test]
fn retained_window_framing_and_copies_grow_linearly() {
    let work = [32, 64, 128].map(|n| {
        crate::tests::WORK.with_borrow_mut(|work| *work = crate::tests::Work::default());
        let mut source = Segmented::from(ScopeSource::identity(""));
        let mut recognizer = rules::paragraph::Recognizer::new(MarkedOptions::Terminal);
        for _ in 0..n {
            source.append(&ScopeSource::identity("é😀 line\n").into());
            recognizer.extend(&source);
        }
        assert_eq!(recognizer.finish(&source).paragraph, Some(source.len() - 1));
        crate::tests::WORK.with_borrow(|work| *work)
    });
    for (n, work) in [32, 64, 128].into_iter().zip(work) {
        assert_eq!(work.framing_bytes, n * "é😀 line\n".len());
    }
    for pair in work.windows(2) {
        assert!(pair[1].joined_bytes <= 3 * pair[0].joined_bytes, "{pair:?}");
        assert!(
            pair[1].paragraph_bytes <= 3 * pair[0].paragraph_bytes,
            "{pair:?}"
        );
    }
}
