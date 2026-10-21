//! Block accumulation ownership before inline publication.
use super::{BlockTokens, Lexer};
use crate::{MarkedOptions, source::ScopeSource, tokenizer};
use std::{collections::HashMap, convert::Infallible};

#[test]
fn block_accumulator_retains_inline_operands_until_publication() {
    for count in [32, 64, 128] {
        let text = format!("a\n{}", "    é\n".repeat(count));
        let source = ScopeSource::identity(&text);
        let mut lexer = Lexer::<Infallible> {
            tokenizer: None,
            error: None,
            top: true,
            clipped: false,
            barriers: None,
            context: tokenizer::inline::LinkContext::new(false),
            options: MarkedOptions::Terminal,
            definitions: HashMap::new(),
            scopes: HashMap::new(),
        };
        let mut output = BlockTokens::default();
        crate::tests::WORK.with_borrow_mut(|work| *work = crate::tests::Work::default());
        for line in crate::lexer::lines(&text) {
            lexer.block_tokens(&source.select(std::iter::once(line)), &mut output, true);
        }
        let copied = crate::tests::WORK.with_borrow(|work| work.source_bytes + work.joined_bytes);
        assert_eq!(output.nodes.len(), 1);
        assert_eq!(output.nodes[0].authored.to_owned_string(), text);
        assert_eq!(copied, 0, "block-phase inline copies at {count}");
        let nodes = lexer.finish_blocks(output);
        let key = (nodes[0].range.start, nodes[0].range.end, false);
        let scope = &lexer.scopes[&key];
        assert_eq!(&*scope.text, format!("a{}", "\né".repeat(count)));
        assert_eq!(scope.native(&(2..4)), Some(6..8));
        assert_eq!(scope.local(&(6..8)), Some(2..4));
        let copied = crate::tests::WORK.with_borrow(|work| work.source_bytes + work.joined_bytes);
        assert_eq!(copied, 1 + 3 * count, "one inline publication at {count}");
    }
}
