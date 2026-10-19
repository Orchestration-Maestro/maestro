//! Credential integration tests compiled as one program.
#![cfg(test)]

mod fallback_support;
mod gate_support;
mod oauth_support;
mod support;

mod accepted_credentials;
mod auth_guidance;
mod config_values;
mod credential_file_backend;
mod credential_key_selection;
mod credential_refresh;
