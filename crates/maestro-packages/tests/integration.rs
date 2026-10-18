//! Shared integration-test executable.
#![cfg(test)]

mod acquisition_checks;
mod available_update_checks;
mod bootstrap_creation_checks;
mod cache_resolution_checks;
#[cfg(not(target_arch = "wasm32"))]
mod cache_support;
mod capture_checks;
#[path = "../../maestro-models/tests/support/child_process.rs"]
mod child_process;
mod component_entry_checks;
#[cfg(not(target_arch = "wasm32"))]
mod component_support;
mod configured_resolution_checks;
#[cfg(unix)]
mod configured_resolution_support;
mod configured_source_checks;
mod configured_update_checks;
mod git_execution_checks;
mod git_ssh_url_checks;
mod git_staging_checks;
mod manifest_expansion_checks;
mod manifest_resource_checks;
#[cfg(unix)]
mod native_git;
#[cfg(unix)]
mod native_git_command;
mod native_support;
mod npm_root_checks;
mod pattern_reexport_checks;
mod pinned_manifest_checks;
mod process_checks;
#[cfg(not(target_arch = "wasm32"))]
mod resolution_support;
#[cfg(not(target_arch = "wasm32"))]
mod resource_cases;
#[cfg(not(target_arch = "wasm32"))]
mod resource_output;
mod resource_resolution_checks;
mod sandbox;
mod scratch;
mod support;
#[cfg(unix)]
mod update_corpus;
#[cfg(unix)]
mod worker_support;
