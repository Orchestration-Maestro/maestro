//! Shared integration-test executable.
#![cfg(test)]

mod author_setup;
mod browser_author_build;
mod browser_export_setup;
mod browser_preparation;
mod build_recipes;
mod cargo_fixtures;
mod component_preparation;
mod consuming_workspace;
mod date_time_data;
mod examples_fetch;
mod fixture_sources_setup;
mod native_setup;
mod nextest_priorities;
mod nextest_wrapper;
mod prepared_setup_packages;
mod setup_docs;
mod shared_ci_callers;
mod tooling_closure;
mod tooling_commands;

use maestro_test_process::compiler;

#[path = "fixtures/recipe_support.rs"]
mod recipe_support;
