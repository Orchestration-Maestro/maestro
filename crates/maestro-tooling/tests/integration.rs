//! Shared integration-test executable.
#![cfg(test)]

mod author_setup;
mod browser_author_build;
mod browser_preparation;
mod build_recipes;
mod cargo_fixtures;
mod component_preparation;
mod fixture_sources_setup;
mod nextest_priorities;
mod prepared_setup_packages;
mod setup_docs;
mod shared_ci_callers;
mod tooling_commands;

#[path = "fixtures/toolchain_support.rs"]
mod toolchain_support;

#[path = "fixtures/recipe_support.rs"]
mod recipe_support;

#[path = "../../maestro-extensions-wasm/build/test_modules.rs"]
mod example_test_modules;
