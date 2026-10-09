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
