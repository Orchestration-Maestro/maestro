#![cfg(test)]
//! Word wrapping through the public editor helper.
use maestro_tui::components::editor::word_wrap_line;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    test: String,
    id: String,
    line: String,
    width: usize,
    parts: Option<Vec<String>>,
    chunks: Vec<Chunk>,
}
#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Chunk {
    text: String,
    start_index: usize,
    end_index: usize,
}
fn run(name: &str) {
    let cases: Vec<Case> = serde_json::from_str(include_str!("fixtures/editor_wrap.json")).unwrap();
    let mut queries = std::collections::HashSet::new();
    let mut count = 0;
    for case in cases.into_iter().filter(|case| case.test == name) {
        assert!(queries.insert((case.line.clone(), case.width, case.parts.clone())));
        let mut offset = 0;
        let parts = case.parts.as_ref().map(|parts| {
            parts
                .iter()
                .map(|text| {
                    let start = offset;
                    offset += text.len();
                    (start, text.as_str())
                })
                .collect::<Vec<_>>()
        });
        let actual = word_wrap_line(&case.line, case.width, parts.as_deref())
            .into_iter()
            .map(|chunk| Chunk {
                text: chunk.text,
                start_index: chunk.start_index,
                end_index: chunk.end_index,
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, case.chunks, "{}", case.id);
        count += 1;
    }
    assert!(count > 0, "missing cases for {name}");
}
#[test]
fn wrap_empty_fit_and_zero_width() {
    run("wrap_empty_fit_and_zero_width");
}
#[test]
fn wrap_breaks_keep_original_ranges() {
    run("wrap_breaks_keep_original_ranges");
}

#[test]
fn wrap_reuses_whitespace_classification() {
    run("wrap_reuses_whitespace_classification");
}

#[test]
fn wrap_preserves_grapheme_boundaries() {
    run("wrap_preserves_grapheme_boundaries");
}

#[test]
fn wrap_supplied_segments_keep_atomic_boundaries() {
    run("wrap_supplied_segments_keep_atomic_boundaries");
}

#[test]
fn wrap_marker_spelling_controls_word_breaks() {
    run("wrap_marker_spelling_controls_word_breaks");
}

#[test]
fn wrap_oversized_grapheme_terminates() {
    run("wrap_oversized_grapheme_terminates");
}

#[test]
fn wrap_keeps_escape_payloads_intact_at_narrow_widths() {
    for escape in ["\x1b[31m", "\x1b]0;x\ty\x07"] {
        let line = format!("a{escape}bc");
        let expected = vec![
            ("a".to_owned(), 0, 1),
            (format!("{escape}b"), 1, 2 + escape.len()),
            ("c".to_owned(), 2 + escape.len(), 3 + escape.len()),
        ];
        let supplied = [(0, line.as_str())];
        for segments in [None, Some(supplied.as_slice())] {
            let chunks = word_wrap_line(&line, 1, segments);
            assert_eq!(
                chunks
                    .into_iter()
                    .map(|part| (part.text, part.start_index, part.end_index))
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }
}

#[test]
fn wrap_supplied_whitespace_excludes_only_escape_payloads() {
    for (line, parts, expected) in [
        (
            "a\x1b]0;x\ty\x07bcd",
            vec!["a\x1b]0;x\ty\x07", "b", "cd"],
            vec!["a\x1b]0;x\ty\x07b", "cd"],
        ),
        (
            "a\x1b[31m bcd",
            vec!["a", "\x1b[31m ", "b", "c", "d"],
            vec!["a\x1b[31m ", "bcd"],
        ),
    ] {
        let mut offset = 0;
        let supplied = parts
            .into_iter()
            .map(|text| {
                let at = offset;
                offset += text.len();
                (at, text)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            word_wrap_line(line, 3, Some(&supplied))
                .into_iter()
                .map(|part| part.text)
                .collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn wrap_escape_interleaved_visible_graphemes_consume_once() {
    for (line, expected) in [
        ("👩\x1b[31m\u{200d}💻x", vec!["👩\x1b[31m\u{200d}💻", "x"]),
        ("\x1b[31m\u{1f3fd}x", vec!["\x1b[31m\u{1f3fd}", "x"]),
        ("ab\x1b[0m", vec!["a", "b\x1b[0m"]),
        ("a \x1b[31mbc", vec!["a", " ", "\x1b[31mb", "c"]),
    ] {
        let supplied = [(0, line)];
        let raw = maestro_tui::get_segmenter(line).collect::<Vec<_>>();
        for segments in [None, Some(supplied.as_slice()), Some(raw.as_slice())] {
            let chunks = word_wrap_line(line, 1, segments);
            assert_eq!(
                chunks
                    .iter()
                    .map(|part| part.text.as_str())
                    .collect::<Vec<_>>(),
                expected
            );
            let mut at = 0;
            for part in chunks {
                assert_eq!(part.start_index, at);
                at += part.text.len();
                assert_eq!(part.end_index, at);
            }
            assert_eq!(at, line.len());
        }
    }
}
