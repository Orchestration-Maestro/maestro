//! Direct continuation-history witnesses at trimmed item boundaries.
use super::{ItemLines, MarkedOptions, ScopeSource};

/// Admit a complete prefix using the same physical-line windows as the list owner.
fn framed(text: &str) -> ItemLines {
    let source = ScopeSource::identity(text);
    let mut lines = crate::lexer::lines(text);
    let first = lines.next().unwrap();
    let marker = super::super::marker(&text[first.clone()]).unwrap();
    let mut item = ItemLines::new(
        &source.select(std::iter::once(first)),
        &marker,
        MarkedOptions::Terminal,
    );
    for line in lines {
        assert!(item.append(&source.select(std::iter::once(line))));
    }
    item
}

#[test]
fn resumed_padding_and_blank_history_follow_the_surviving_raw() {
    for (prefix, joined, expected) in [
        ("- a ", "- a \nlazy", "a \nlazy"),
        ("- a\n  p\t", "- a\n  p\t\n  q", "a\np    \nq"),
        ("- a\np\t", "- a\np\t\nq", "a\np\t\nq"),
        ("- a\n  p\n  \n", "- a\n  p\nlazy", "a\np\nlazy"),
        ("- a\n  p\n", "- a\n  p\n\n  q", "a\np\n\nq"),
    ] {
        let mut item = framed(prefix);
        let source = ScopeSource::identity(joined);
        let offset = item.resume(&source.clone().into(), 0);
        for line in crate::lexer::lines(&joined[offset..]) {
            assert!(
                item.append(
                    &source.select(std::iter::once(offset + line.start..offset + line.end,))
                )
            );
        }
        assert_eq!(
            item.body().mapped().to_owned_string(),
            expected,
            "{prefix:?}"
        );
        assert_eq!(item.raw_end(), joined.trim_end().len());
    }
}

#[test]
fn resumed_empty_marker_and_first_line_code_keep_their_stop_rules() {
    let mut empty = framed("-");
    let source = ScopeSource::identity("-\n\nq");
    let offset = empty.resume(&source.clone().into(), 0);
    assert_eq!(offset, 2);
    assert!(empty.append(&source.select(std::iter::once(2..3))));
    assert!(!empty.append(&source.select(std::iter::once(3..4))));
    assert_eq!(empty.body().len(), 0);

    let mut code = framed("-    a");
    let source = ScopeSource::identity("-    a\nlazy");
    let offset = code.resume(&source.clone().into(), 0);
    assert!(!code.append(&source.select(std::iter::once(offset..source.text.len()))));
}
