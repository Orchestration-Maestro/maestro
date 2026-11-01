//! Markdown integration tests compiled as one program.
#![cfg(test)]

mod container_accumulation;
mod deletion_scanning;
mod grammar_rules;
#[path = "grammar_rules/observations.rs"]
mod observations;
mod parsing;
