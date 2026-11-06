//! Container release and retained quote resumption state.
use super::{QuoteFrame, Resume, forget, quote_prefix};
use crate::lexer::{BlockTokens, Level, blocks::tests::lexer};
use crate::source::{Cursor, ScopeSource, Segmented};
use crate::tests::{REDUCED_STACK, on_stack};
use crate::{Kind, MarkedOptions, parse_with_options};
use std::collections::HashMap;

#[test]
fn forget_releases_a_parsed_container_chain_on_a_small_stack() {
    on_stack(REDUCED_STACK, || {
        let document = parse_with_options(
            format!("{} deep", ">".repeat(2_000)),
            MarkedOptions::Terminal,
        );
        let mut deepest = &document.nodes[0];
        while let Kind::Quote(children, _) = &deepest.kind {
            deepest = &children[0];
        }
        let key = (deepest.range.start, deepest.range.end, false);
        let mut scopes = HashMap::from([(key, 1)]);
        forget(&mut scopes, &document.nodes[0]);
        assert!(scopes.is_empty());
    });
}

#[test]
fn quote_reentry_truncates_at_the_authored_container_start() {
    let source = ScopeSource::identity("lead\n\n> a\n> b");
    let mut lexer = lexer();
    let mut frame = QuoteFrame::new(source.clone().into());
    frame.text = source.into();
    frame.pending = Cursor::default();
    frame.output = lexer.append(frame.text.clone(), BlockTokens::default(), Level::Chunk);

    assert!(matches!(
        lexer.resume_container(&mut frame),
        Some(Resume::Quote(_))
    ));
    assert_eq!(frame.text.mapped().to_owned_string(), "lead\n\n");
}

/// A spliced operand: one quoted line, then `rest` from a second allocation.
fn spliced_quote(rest: &str) -> Segmented {
    let mut source: Segmented = ScopeSource::identity("> a\n").into();
    source.append(&ScopeSource::identity(rest).into());
    source
}

#[test]
fn quote_prefix_growth_copies_each_spliced_byte_once() {
    let rule = crate::rules::block_blockquote(MarkedOptions::Terminal).unwrap();
    for rest in [
        "> b\n".repeat(1_000),
        "> b\n".repeat(100) + &"\nplain\n".repeat(1_000),
        "> b\nlazy\n".repeat(500),
        "> b\n".to_owned(),
    ] {
        let source = spliced_quote(&rest);
        let whole = source.materialize();
        crate::tests::WORK.with_borrow_mut(|work| *work = crate::tests::Work::default());

        let (raw, matched) = quote_prefix(&source, rule).unwrap();

        let (copied, recognized) =
            crate::tests::WORK.with_borrow(|work| (work.source_bytes, work.block_recognizer_bytes));
        assert_eq!(copied, raw.text.len(), "{}", &rest[..10.min(rest.len())]);
        // Matching every doubled prefix from its start reads each byte about twice.
        assert!(
            2 * recognized <= 3 * raw.text.len(),
            "recognized {recognized} for {} bytes",
            raw.text.len()
        );
        assert_eq!(&*raw.text, &whole.text[..raw.text.len()]);
        assert_eq!(matched, rule.find(&whole.text).unwrap().end());
    }
}
