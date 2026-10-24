//! Release of a deep container chain built by the parser.
use super::forget;
use crate::tests::{MULTIBYTE_REENTRY_BODY, MULTIBYTE_REENTRY_SOURCE, REDUCED_STACK, on_stack};
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
    let document = parse_with_options(MULTIBYTE_REENTRY_SOURCE.to_owned(), MarkedOptions::Terminal);
    let Kind::Quote(outer, _) = &document.nodes[1].kind else {
        panic!("outer quote")
    };
    let Kind::List(_, items) = &outer[0].kind else {
        panic!("quoted list")
    };
    let Kind::Item(children) = &items[0].kind else {
        panic!("quoted item")
    };
    let Kind::Quote(container, container_body) = &children[1].kind else {
        panic!("container quote")
    };
    assert_eq!(container_body.to_owned_string(), MULTIBYTE_REENTRY_BODY);
    let Kind::Quote(_, reentered_body) = &container[0].kind else {
        panic!("re-entered quote")
    };
    assert_eq!(reentered_body.to_owned_string(), MULTIBYTE_REENTRY_BODY);
}
