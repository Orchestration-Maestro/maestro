//! Shared integration-test executable.
#![cfg(test)]

mod catalog_auth_metadata;
mod catalog_composition;
mod catalog_errors;
mod catalog_loading;
mod catalog_observations;
mod catalog_provider_affinity;
mod catalog_provider_lifecycle;
mod catalog_provider_registration;
mod catalog_reentry;
mod catalog_request_auth;
mod cloudflare_auth;
#[allow(
    dead_code,
    reason = "This binary uses part of the shared credential fallback corpus."
)]
#[path = "../../maestro-credentials/tests/fallback_support/mod.rs"]
mod credential_fallback;
#[path = "support/descriptors.rs"]
mod descriptors;
#[path = "support/dynamic.rs"]
mod dynamic;
#[path = "support/fallback.rs"]
mod fallback;
#[path = "support/isolation.rs"]
mod isolation;
mod model_cli_resolution;
mod model_file_layers;
mod model_initial_choice;
mod model_locale_ordering;
mod model_reference_matching;
mod model_restoration;
mod model_scopes;
#[path = "support/request.rs"]
#[cfg(not(target_arch = "wasm32"))]
mod request_support;
mod saved_model_owner;
#[path = "selection/scopes.rs"]
mod scopes;
mod selection;
#[path = "support/subprocess.rs"]
mod subprocess;
mod support;
