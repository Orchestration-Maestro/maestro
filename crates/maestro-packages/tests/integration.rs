//! Shared integration-test executable.
#![cfg(test)]

mod acquisition_checks;
mod available_update_checks;
mod bootstrap_creation_checks;
mod cache_resolution_checks;
#[cfg(not(target_arch = "wasm32"))]
mod cache_support;
mod candidate_inspection_checks;
mod capture_checks;
mod capture_context_checks;
mod capture_path_checks;
mod component_entry_checks;
#[cfg(not(target_arch = "wasm32"))]
mod component_support;
mod configured_resolution_checks;
#[cfg(unix)]
mod configured_resolution_support;
mod configured_source_checks;
mod configured_update_checks;
mod contribution_resource_checks;
mod git_execution_checks;
mod git_reinstall_checks;
mod git_ssh_url_checks;
mod git_staging_checks;
mod manifest_case_checks;
mod manifest_expansion_checks;
mod manifest_resource_checks;
#[cfg(unix)]
mod native_git;
#[cfg(unix)]
mod native_git_command;
mod native_support;
mod npm_root_checks;
mod pattern_reexport_checks;
mod pin_integrity_checks;
mod pin_safety_checks;
mod pinned_manifest_checks;
mod process_checks;
mod release_inspection_checks;
#[cfg(not(target_arch = "wasm32"))]
mod resolution_support;
mod resolved_package_checks;
#[cfg(not(target_arch = "wasm32"))]
mod resource_cases;
#[cfg(not(target_arch = "wasm32"))]
mod resource_output;
mod resource_resolution_checks;
#[path = "support/runtime.rs"]
mod runtime;
mod sandbox;
mod scratch;
mod skill_pack_checks;
mod startup_environment;
mod support;
#[cfg(unix)]
mod update_corpus;
#[cfg(unix)]
mod worker_support;
