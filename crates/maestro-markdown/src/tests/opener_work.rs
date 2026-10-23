//! Work each reached opener supplies to the shared delimiter owner.
use super::{SELECTIONS, WORK, Work, assert_literal, parsed};
use crate::{Kind, MarkedOptions, source::ScopeSource};
use std::collections::HashMap;

/// One recognizer family: its unclosed-opener workload, its counter and its bound.
type Family = (fn(usize) -> String, fn(&Work) -> usize, usize);

/// Families whose unclosed openers must read each source byte a fixed number of times.
///
/// A bound is the passes one preparation makes over a source view, plus at most one
/// query per source byte: one pass for the code-span runs and two for the label scan.
const FAMILIES: [Family; 3] = [
    (distinct_widths, |work: &Work| work.inline_code_bytes, 2),
    (unclosed_links, |work: &Work| work.link_bytes, 2),
    (escaped_openers, |work: &Work| work.link_bytes, 3),
];

/// One unclosed code-span opener per distinct backtick width.
fn distinct_widths(count: usize) -> String {
    (1..=count).map(|width| "`".repeat(width) + "a ").collect()
}

/// Unclosed direct-link openers with no `](` anywhere.
fn unclosed_links(count: usize) -> String {
    "[b ".repeat(count)
}

/// Escaped bracket openers beside a closer the scan never reaches.
fn escaped_openers(count: usize) -> String {
    "\\[x\\]( ".repeat(count)
}

/// Work one workload supplies to the selected counter through the private tokenizer.
fn root_work(text: &str, options: MarkedOptions, counter: fn(&Work) -> usize) -> usize {
    WORK.with_borrow_mut(|work| *work = Work::default());
    let source = ScopeSource::identity(text);
    let definitions = HashMap::new();
    let mut tokenizer =
        crate::tokenizer::inline::Tokenizer::new(&source, options, &definitions, None, false);
    let nodes = tokenizer.parse(&[], None);
    assert_literal(&nodes, text, &format!("{options:?}"));
    WORK.with_borrow(counter)
}

/// Work one workload supplies to the selected counter through both public entries.
fn parse_work(
    text: &str,
    options: MarkedOptions,
    extension: bool,
    counter: fn(&Work) -> usize,
) -> usize {
    WORK.with_borrow_mut(|work| *work = Work::default());
    let document = parsed(text.to_owned(), options, extension);
    assert!(
        matches!(
            document.nodes.first().map(|node| &node.kind),
            Some(Kind::Paragraph(_))
        ),
        "{options:?}"
    );
    if let Some(Kind::Paragraph(children)) = document.nodes.first().map(|node| &node.kind) {
        assert_literal(children, text, &format!("{options:?}"));
    }
    WORK.with_borrow(counter)
}

/// Work and byte measures of one workload at each recorded size.
fn measured(
    workload: fn(usize) -> String,
    work: impl Fn(&str) -> usize,
) -> ([usize; 3], [usize; 3]) {
    let texts = [32, 64, 128].map(workload);
    let bytes = texts.each_ref().map(String::len);
    (texts.each_ref().map(|text| work(text)), bytes)
}

/// Assert each workload size against its bound and report the measured work.
fn assert_bounded(label: &str, work: [usize; 3], bytes: [usize; 3], bound: usize) {
    eprintln!("{label} work {work:?} bytes {bytes:?}");
    for (work, bytes) in work.into_iter().zip(bytes) {
        assert!(work <= bound * bytes, "{label} {work} > {bound} * {bytes}");
    }
}

/// Assert one family's root work against its bound under one selection.
fn check_root_work(
    options: MarkedOptions,
    workload: fn(usize) -> String,
    counter: fn(&Work) -> usize,
    bound: usize,
) {
    let (work, bytes) = measured(workload, |text| root_work(text, options, counter));
    assert_bounded(&format!("OPENER ROOT {options:?}"), work, bytes, bound);
}

/// Assert every family's work against its bound through one parse route.
fn check_parse_work(options: MarkedOptions, extension: bool) {
    for (workload, counter, bound) in FAMILIES {
        let work = |text: &str| parse_work(text, options, extension, counter);
        let (work, bytes) = measured(workload, work);
        let label = format!("OPENER PARSE {options:?} extension={extension}");
        assert_bounded(&label, work, bytes, bound);
    }
}

#[test]
fn opener_work_stays_within_source_bytes() {
    for options in SELECTIONS {
        for (workload, counter, bound) in FAMILIES {
            check_root_work(options, workload, counter, bound);
        }
    }
}

#[test]
fn opener_work_is_linear_through_parse() {
    for options in SELECTIONS {
        for extension in [false, true] {
            check_parse_work(options, extension);
        }
    }
}
