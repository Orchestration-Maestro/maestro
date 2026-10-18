//! Shared integration-test executable.
#![cfg(test)]

mod architecture;
mod boundaries;
#[cfg(unix)]
#[path = "../../maestro-models/tests/support/child_process.rs"]
mod child_process;
mod comments;
mod dependency_locks;
mod development_guidance;
mod integration_tests;
mod json_dependencies;
mod quality;
mod repetition;
mod support;
mod time_limits;
