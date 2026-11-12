//! Items that take the aligned lines their enclosing item recorded admit exactly what
//! line-by-line admission admits.
use super::{
    container_depth::{indented, recorded},
    line_by_line,
};
use crate::{MarkedOptions, parse_with_options};

/// Physical lines the generated documents are made of: list markers at several
/// indentations, blank and tab-spelled lines, lazy text, quotes, fences and tasks.
const LINES: [&str; 16] = [
    "- a", "- a", "- b", "1. c", "text", "", "> q", "```", "  ", "- [x] t", "- a", "\tt", "- b",
    "   ", "-", "  \u{a0}",
];

/// Leading spaces tried before each generated line.
const INDENTS: [usize; 8] = [0, 2, 2, 4, 4, 6, 1, 3];

/// Documents of up to ten lines drawn from `LINES` and `INDENTS` by a fixed generator.
fn documents(count: usize) -> Vec<String> {
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    let mut next = move |bound: usize| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 33) as usize % bound
    };
    (0..count)
        .map(|_| {
            let lines = 2 + next(9);
            let mut source = String::new();
            for _ in 0..lines {
                source.push_str(&" ".repeat(INDENTS[next(INDENTS.len())]));
                source.push_str(LINES[next(LINES.len())]);
                source.push_str(["\n", "\n", "\r\n"][next(3)]);
            }
            if next(4) == 0 {
                source.pop();
            }
            source
        })
        .collect()
}

/// Require run adoption and line-by-line admission to record the same tree for `source`,
/// returning the lines the run adoption took.
fn assert_fast_matches_slow(source: &str, options: MarkedOptions) -> usize {
    super::WORK.with_borrow_mut(|work| *work = super::Work::default());
    let fast = recorded(&parse_with_options(source.to_owned(), options));
    let adopted = super::WORK.with_borrow(|work| work.list_adopted);
    let slow = line_by_line(|| recorded(&parse_with_options(source.to_owned(), options)));
    assert_eq!(fast, slow, "{source:?} {options:?}");
    adopted
}

#[test]
fn adopted_continuation_lines_match_line_by_line_admission() {
    let mut adopted = 0;
    for options in [
        MarkedOptions::Terminal,
        MarkedOptions::Browser { escape_html: true },
    ] {
        for source in documents(1_500) {
            adopted += assert_fast_matches_slow(&source, options);
        }
    }
    assert!(adopted > 0, "no generated document adopted a run");
}

#[test]
fn adopted_lines_keep_the_blank_state_and_the_enclosing_boundaries() {
    for source in [
        "- a\n\n    - b\nc\n",
        "- a\n  - b\n\n  - c\n",
        "- a\n  - b\n    - c\n  d\n",
        "- a\n  - b\n    - c\nd\n",
        "1. a\n   - b\n     lazy\n     - c\n",
        "- a\n  - b\n  \n    - c\n   x\n",
        "- a\n  - b\n\t- c\n  - d\n",
        "-\n  - b\n    - c\n",
    ] {
        assert_fast_matches_slow(source, MarkedOptions::Terminal);
    }
}

/// Continuation lines decided and adopted by one parse of `source`.
fn admissions(source: &str) -> (usize, usize) {
    super::WORK.with_borrow_mut(|work| *work = super::Work::default());
    let _ = parse_with_options(source.to_owned(), MarkedOptions::Terminal);
    super::WORK.with_borrow(|work| (work.list_lines, work.list_adopted))
}

#[test]
fn every_continuation_line_is_decided_or_adopted_once() {
    let source = indented(40, "");
    let (decided, adopted) = admissions(&source);
    let (every, none) = line_by_line(|| admissions(&source));
    assert_eq!(none, 0);
    assert!(adopted > 0, "no run was adopted");
    assert_eq!(decided + adopted, every);
    assert!(decided < every / 4, "decided {decided} of {every}");
}
