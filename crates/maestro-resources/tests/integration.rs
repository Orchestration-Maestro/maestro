//! Shared integration-test executable.
#![cfg(test)]

#[cfg(unix)]
#[path = "../../maestro-models/tests/support/child_process.rs"]
mod child_process;
mod development_guidance;
mod frontmatter_loading;
mod frontmatter_values;
mod resource_paths;
mod skill_discovery;
mod skill_ignore_checks;
mod support;
mod template_expansion;
mod working_directory;
