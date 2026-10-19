//! Shared integration-test executable.
#![cfg(test)]

mod build_recipes;
mod tooling_commands;

#[path = "fixtures/toolchain_support.rs"]
mod toolchain_support;
