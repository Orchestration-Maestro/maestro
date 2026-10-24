//! Block accumulation ownership before inline publication.
use super::{BlockTokens, Level, Lexer};
use crate::{MarkedOptions, source::ScopeSource, tokenizer};
use std::{collections::HashMap, convert::Infallible};

/// One source-only lexer using the ordinary block dispatcher.
pub(in crate::lexer) fn lexer() -> Lexer<'static, Infallible> {
    Lexer {
        tokenizer: None,
        error: None,
        top: true,
        clipped: false,
        undecided: false,
        context: tokenizer::inline::LinkContext::new(false),
        options: MarkedOptions::Terminal,
        definitions: HashMap::new(),
        scopes: HashMap::new(),
    }
}

#[test]
fn list_children_publish_scopes_at_their_invocation_boundary() {
    let source = ScopeSource::identity("- é\n- [X] β");
    let mut lexer = lexer();
    let output = lexer.append(source.into(), BlockTokens::default(), Level::Document);
    assert!(lexer.scopes.is_empty(), "provisional item scopes escaped");
    let nodes = lexer.finish_blocks(output);
    assert_eq!(lexer.scopes.len(), 2);
    let crate::Kind::List(_, items) = &nodes[0].kind else {
        panic!("list")
    };
    let crate::Kind::Item(first) = &items[0].kind else {
        panic!("first item")
    };
    let crate::Kind::Item(second) = &items[1].kind else {
        panic!("second item")
    };
    assert!(matches!(first[0].kind, crate::Kind::Inline(_)));
    assert!(matches!(second[0].kind, crate::Kind::TaskListMarker(true)));
    assert_eq!(second[0].range, 13..13);
    for (child, text) in [(&first[0], "é"), (&second[1], "β")] {
        let scope = &lexer.scopes[&(child.range.start, child.range.end, false)];
        assert_eq!(&*scope.text, text);
        assert_eq!(scope.native(&(0..text.len())), Some(child.range.clone()));
    }
}

#[test]
fn block_accumulator_retains_inline_operands_until_publication() {
    for count in [32, 64, 128] {
        let text = format!("a\n{}", "    é\n".repeat(count));
        let source = ScopeSource::identity(&text);
        let mut lexer = lexer();
        crate::tests::WORK.with_borrow_mut(|work| *work = crate::tests::Work::default());
        let mut output = BlockTokens::default();
        for line in crate::lexer::lines(&text) {
            output = lexer.append(
                source.select(std::iter::once(line)).into(),
                output,
                Level::Chunk,
            );
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

#[test]
fn block_dispatch_bounds_operands_to_the_admitted_leaf() {
    let counts = [32, 64, 128].map(|count| {
        crate::tests::WORK.with_borrow_mut(|work| *work = crate::tests::Work::default());
        let document = crate::parse("# é\n\n***\n\n".repeat(count));
        assert_eq!(
            document
                .nodes
                .iter()
                .filter(|node| matches!(node.kind, crate::Kind::Heading(..)))
                .count(),
            count,
        );
        assert_eq!(
            document
                .nodes
                .iter()
                .filter(|node| matches!(node.kind, crate::Kind::Rule))
                .count(),
            count,
        );
        crate::tests::WORK.with_borrow(|work| work.block_recognizer_bytes)
    });
    eprintln!("bounded block dispatch operands: {counts:?}");
    assert!(counts.windows(2).all(|pair| pair[1] <= 3 * pair[0]));
}

/// Block output whose trailing list still carries its pending item history.
fn pending_list(text: &'static str) -> BlockTokens {
    let mut lexer = lexer();
    lexer.append(
        ScopeSource::identity(text).into(),
        BlockTokens::default(),
        Level::Document,
    )
}

#[test]
#[should_panic(expected = "a published list ends with the node carrying its item slots")]
fn list_publication_reports_a_missing_list_node() {
    let mut output = pending_list("- a\n- b\n");
    let mut node = output.nodes.pop().expect("the published list node");
    node.kind = crate::Kind::Paragraph(Vec::new());
    output.nodes.push(node);
    output.complete();
}

#[test]
#[should_panic(expected = "a published list ends with the node carrying its item slots")]
fn list_publication_reports_an_output_without_nodes() {
    let mut output = pending_list("- a\n- b\n");
    output.nodes.clear();
    output.complete();
}

#[test]
#[should_panic(expected = "every item slot of a list is filled by exactly one output")]
fn list_publication_reports_outputs_without_their_item_slots() {
    let mut output = pending_list("- a\n- b\n");
    let crate::Kind::List(_, items) = &mut output.nodes.last_mut().expect("list").kind else {
        panic!("list")
    };
    items.truncate(1);
    output.complete();
}

#[test]
#[should_panic(expected = "every item slot of a list is filled by exactly one output")]
fn list_publication_reports_item_slots_without_their_outputs() {
    let mut output = pending_list("para\n\n- a\n- b\n");
    let mut spare = output.nodes.remove(0);
    spare.kind = crate::Kind::Item(Vec::new());
    let crate::Kind::List(_, items) = &mut output.nodes.last_mut().expect("list").kind else {
        panic!("list")
    };
    items.push(spare);
    output.complete();
}

#[test]
fn list_publication_pairs_every_item_slot_with_an_output() {
    let mut output = pending_list("para\n\n- a\n");
    let mut spare = output.nodes.remove(0);
    spare.kind = crate::Kind::Item(Vec::new());
    let crate::Kind::List(_, mut items) = std::mem::replace(
        &mut output.nodes.last_mut().expect("list").kind,
        crate::Kind::Gap,
    ) else {
        panic!("list")
    };
    items.push(spare);
    let paired: Vec<_> = super::publish::pair_slots(items, vec![BlockTokens::default()]).collect();
    assert_eq!(paired.len(), 2, "a slot with no output keeps its place");
    assert!(
        paired[1].1.nodes.is_empty(),
        "a slot with no output is empty"
    );
}

#[test]
fn list_publication_pairs_no_slot_with_an_output_it_does_not_have() {
    let mut output = pending_list("- a\n");
    let crate::Kind::List(_, items) = std::mem::replace(
        &mut output.nodes.last_mut().expect("list").kind,
        crate::Kind::Gap,
    ) else {
        panic!("list")
    };
    let outputs = vec![BlockTokens::default(), BlockTokens::default()];
    let paired: Vec<_> = super::publish::pair_slots(items, outputs).collect();
    assert_eq!(
        paired.len(),
        1,
        "an output without a slot has nothing to fill"
    );
}

#[test]
#[should_panic(expected = "a list's item slot holds an item node")]
fn list_publication_reports_a_slot_that_is_not_an_item() {
    let mut output = pending_list("- a\n- b\n");
    let crate::Kind::List(_, items) = &mut output.nodes.last_mut().expect("list").kind else {
        panic!("list")
    };
    items[0].kind = crate::Kind::Gap;
    output.complete();
}
