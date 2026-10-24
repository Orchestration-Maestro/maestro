//! Shared integration-test executable.
#![cfg(test)]

mod browser_author_build;
mod build_recipes;
mod tooling_commands;

use maestro_test_process::toolchain as toolchain_support;

#[path = "fixtures/recipe_support.rs"]
mod recipe_support;
