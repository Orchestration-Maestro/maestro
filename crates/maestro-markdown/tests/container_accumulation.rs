#![cfg(test)]
//! Recursive container observations through each configured parser route.
#[path = "grammar_rules/observations.rs"]
mod observations;

#[test]
fn container_tables_follow_ordered_dispatch() {
    observations::check(
        "container_tables_follow_ordered_dispatch",
        5,
        include_str!("fixtures/container_accumulation.json"),
    );
}
