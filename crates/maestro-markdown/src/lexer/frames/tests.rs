//! Release of a deep container chain built by the parser.
use super::forget;
use crate::{Kind, MarkedOptions, parse_with_options};
use std::collections::HashMap;

/// Worker stack that makes one native frame per container observable at a parsed
/// depth debug can afford.
const WORKER_STACK: usize = 128 * 1024;

#[test]
fn forget_releases_a_parsed_container_chain_on_a_small_stack() {
    std::thread::Builder::new()
        .stack_size(WORKER_STACK)
        .spawn(|| {
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
        })
        .expect("worker thread")
        .join()
        .expect("worker panicked");
}
