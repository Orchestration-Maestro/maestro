//! Production line counting over located documentation and test spans.

use std::path::PathBuf;

use super::production_lines;
use crate::source::{Member, SPAN_TESTS, Source};

/// Count the span containment tests that counting one documented source's lines performs.
fn span_tests(items: usize) -> usize {
    let member = Member {
        name: "maestro-models".into(),
        directory: PathBuf::from("models"),
        sources: vec![Source::new(
            PathBuf::from("models/src/lib.rs"),
            "/// Item.\npub const ITEM: u8 = 0;\n".repeat(items),
        )],
    };
    SPAN_TESTS.set(0);
    assert_eq!(production_lines(&member, &member.sources[0]), items);
    SPAN_TESTS.get()
}

#[test]
fn production_line_span_tests_grow_linearly() {
    let small = span_tests(64);
    let large = span_tests(128);
    println!("span tests: N={small}, 2N={large}");
    assert_eq!(
        large,
        2 * small,
        "doubling documented items doubles span tests"
    );
}
