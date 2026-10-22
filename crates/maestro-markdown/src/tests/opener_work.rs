//! Work each reached opener supplies to the shared delimiter owner.
use super::{SELECTIONS, WORK, Work, assert_literal, parsed};
use crate::{Kind, MarkedOptions, source::ScopeSource};
use std::collections::HashMap;

/// One unclosed code-span opener per distinct backtick width.
fn distinct_widths(count: usize) -> String {
    (1..=count).map(|width| "`".repeat(width) + "a ").collect()
}

/// Unclosed direct-link openers with no `](` anywhere.
fn unclosed_links(count: usize) -> String {
    "[b ".repeat(count)
}

/// Prepared-owner work for one source, through the private tokenizer.
fn root_work(text: &str, options: MarkedOptions) -> usize {
    WORK.with_borrow_mut(|work| *work = Work::default());
    let source = ScopeSource::identity(text);
    let definitions = HashMap::new();
    let mut tokenizer =
        crate::tokenizer::inline::Tokenizer::new(&source, options, &definitions, None, false);
    let nodes = tokenizer.parse(&[], None);
    assert_literal(&nodes, text, &format!("{options:?}"));
    WORK.with_borrow(|work| work.inline_code_bytes)
}

/// Prepared-owner link work for one source, through the private tokenizer.
fn root_link_work(text: &str, options: MarkedOptions) -> usize {
    WORK.with_borrow_mut(|work| *work = Work::default());
    let source = ScopeSource::identity(text);
    let definitions = HashMap::new();
    let mut tokenizer =
        crate::tokenizer::inline::Tokenizer::new(&source, options, &definitions, None, false);
    let nodes = tokenizer.parse(&[], None);
    assert_literal(&nodes, text, &format!("{options:?}"));
    WORK.with_borrow(|work| work.link_bytes)
}

#[test]
fn code_span_opener_work_stays_within_source_bytes() {
    for options in SELECTIONS {
        let sizes = [32, 64, 128];
        let texts = sizes.map(distinct_widths);
        let work = texts.each_ref().map(|text| root_work(text, options));
        let bytes = texts.each_ref().map(String::len);
        eprintln!("CODE SPAN ROOT {options:?} work {work:?} bytes {bytes:?}");
        for (work, bytes) in work.into_iter().zip(bytes) {
            assert!(work <= 2 * bytes, "{options:?} {work} > 2 * {bytes}");
        }
    }
}

#[test]
fn code_span_opener_work_is_linear_through_parse() {
    for options in SELECTIONS {
        for extension in [false, true] {
            let work = [32, 64, 128].map(|count| {
                let text = distinct_widths(count);
                WORK.with_borrow_mut(|state| *state = Work::default());
                let document = parsed(text, options, extension);
                let kind = &document.nodes[0].kind;
                assert!(matches!(kind, Kind::Paragraph(_)), "{options:?}");
                WORK.with_borrow(|state| state.inline_code_bytes)
            });
            eprintln!("CODE SPAN PARSE {options:?} extension={extension} work {work:?}");
            assert!(work[1] <= 4 * work[0], "{options:?} {work:?}");
            assert!(work[2] <= 4 * work[1], "{options:?} {work:?}");
        }
    }
}

#[test]
fn link_admission_work_stays_within_source_bytes() {
    for options in SELECTIONS {
        let texts = [32, 64, 128].map(unclosed_links);
        let work = texts.each_ref().map(|text| root_link_work(text, options));
        let bytes = texts.each_ref().map(String::len);
        eprintln!("LINK ROOT {options:?} work {work:?} bytes {bytes:?}");
        for (work, bytes) in work.into_iter().zip(bytes) {
            assert!(work <= 2 * bytes, "{options:?} {work} > 2 * {bytes}");
        }
        assert!(work[1] <= 4 * work[0] && work[2] <= 4 * work[1]);
    }
}

#[test]
fn link_admission_work_is_linear_through_parse() {
    for options in SELECTIONS {
        for extension in [false, true] {
            let work = [32, 64, 128].map(|count| {
                let text = unclosed_links(count);
                WORK.with_borrow_mut(|state| *state = Work::default());
                let document = parsed(text, options, extension);
                assert!(matches!(document.nodes[0].kind, Kind::Paragraph(_)));
                WORK.with_borrow(|state| state.link_bytes)
            });
            eprintln!("LINK PARSE {options:?} extension={extension} work {work:?}");
            assert!(work[1] <= 4 * work[0], "{options:?} {work:?}");
            assert!(work[2] <= 4 * work[1], "{options:?} {work:?}");
        }
    }
}
