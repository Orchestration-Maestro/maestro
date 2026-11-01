//! Container release and retained quote resumption state.
use super::{QuoteFrame, Resume, forget};
use crate::lexer::{BlockTokens, Level, blocks::tests::lexer};
use crate::source::{Cursor, ScopeSource};
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
