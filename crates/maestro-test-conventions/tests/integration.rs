//! Shared integration-test executable.
#![cfg(test)]

mod application_exports;
mod architecture;
mod boundaries;
mod bucket_modules;
mod child_tests;
mod comments;
mod config_scripts;
mod dependency_locks;
mod development_guidance;
mod documented_commands;
mod duplicates;
mod fixture_includes;
mod integration_tests;
mod json_dependencies;
mod leaked_words;
mod lib_tests;
mod message_renderer_ownership;
mod package_crates;
mod platform_opener;
mod poison;
mod quality;
mod real_paths;
mod repetition;
mod rpc_ownership;
mod setup_children;
mod standard_error;
mod support;
mod temp_directories;
mod test_directories;
mod test_input_routes;
mod time_limits;
mod tokio_macros;
mod tracked_scripts;
mod tracked_sizes;

mod test_budget;
