//! Shared integration-test executable.
#![cfg(test)]

mod application_exports;
mod architecture;
mod boundaries;
mod child_tests;
mod comments;
mod config_scripts;
mod dependency_locks;
mod development_guidance;
mod duplicates;
mod example_helpers;
mod integration_tests;
mod json_dependencies;
mod message_renderer_ownership;
mod quality;
mod repetition;
mod rpc_ownership;
mod support;
mod temp_directories;
mod test_directories;
mod time_limits;

mod test_budget;
