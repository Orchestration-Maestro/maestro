//! Work bounds through the public inline parser.
use super::{SELECTIONS, WORK, Work};
use crate::{MarkedOptions, parse_with_options};

/// Observe real parser work after resetting this thread's counters.
fn measured(source: String, options: MarkedOptions) -> Work {
    WORK.with_borrow_mut(|work| *work = Work::default());
    let document = parse_with_options(source, options);
    assert_eq!(document.nodes.len(), 1);
    WORK.with_borrow(|work| *work)
}

/// Allow less work while bounding every sample by its recorded traversal envelope.
fn within_bounds(actual: [usize; 3], bounds: [usize; 3]) {
    for (actual, bound) in actual.into_iter().zip(bounds) {
        assert!(actual <= bound, "work {actual} exceeds {bound}");
    }
}

#[test]
fn emphasis_opening_run_work_is_linear() {
    for options in SELECTIONS {
        for delimiter in ['*', '_'] {
            let work = [32, 64, 128].map(|n| {
                measured(
                    format!("Output: {}a ", delimiter.to_string().repeat(n)),
                    options,
                )
                .emphasis_left
            });
            eprintln!("OPENING {options:?} {delimiter}: {work:?}");
            within_bounds(work, [42, 74, 138]);
        }
    }
}

#[test]
fn paired_delimiter_metadata_work_is_linear() {
    let work = [32, 64, 128].map(|n| {
        measured(
            format!("Output: {}a{}", "*".repeat(n), "*".repeat(n)),
            MarkedOptions::Terminal,
        )
        .emphasis_left
    });
    eprintln!("PAIRED METADATA {work:?}");
    within_bounds(work, [73, 137, 265]);
}

#[test]
fn emphasis_right_suffix_work_matches_reference_growth() {
    for options in SELECTIONS {
        for delimiter in ['*', '_'] {
            let work = [32, 64, 128].map(|n| {
                measured(
                    format!(
                        "Output: {}a {}",
                        delimiter.to_string().repeat(n),
                        "b ".repeat(n)
                    ),
                    options,
                )
                .emphasis_right
            });
            eprintln!("RIGHT SUFFIX {options:?} {delimiter}: {work:?}");
            within_bounds(work, [2080, 8256, 32896]);
        }
    }
}

#[test]
fn repeated_openers_work_matches_reference_growth() {
    let work = [32, 64, 128].map(|n| {
        measured(
            format!("Output: {}", "*a ".repeat(n)),
            MarkedOptions::Terminal,
        )
        .emphasis_right
    });
    eprintln!("REPEATED OPENERS {work:?}");
    within_bounds(work, [1520, 6112, 24512]);
}

#[test]
fn paired_emphasis_mask_work_matches_reference_growth() {
    let work = [32, 64, 128].map(|n| {
        measured(
            format!("Output: {}a{}", "*".repeat(n), "*".repeat(n)),
            MarkedOptions::Terminal,
        )
        .mask_bytes
    });
    eprintln!("PAIRED MASK {work:?}");
    within_bounds(work, [3 * 569, 3 * 2153, 3 * 8393]);
}

#[test]
fn code_span_and_link_openers_stay_within_recorded_work() {
    for options in SELECTIONS {
        let code = [32, 64, 128].map(|n| {
            let source: String = (1..=n).map(|width| "`".repeat(width) + "a ").collect();
            measured(source, options).inline_code_bytes
        });
        eprintln!("CODE SPAN OPENERS {options:?}: {code:?}");
        within_bounds(code, [12_496, 93_600, 723_776]);
        let link = [32, 64, 128].map(|n| measured("[b ".repeat(n), options).link_bytes);
        eprintln!("LINK OPENERS {options:?}: {link:?}");
        within_bounds(link, [1_584, 6_240, 24_768]);
    }
}
