//! Shared integration-test executable.
#![cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
use maestro_http::test_support::runtime;
mod api_registry_base;
mod assistant_publication;
#[path = "support/chat.rs"]
mod chat;
mod checked_tool_arguments;
#[path = "support/clock.rs"]
mod clock;
mod compatibility_invocation;
mod context_overflow_classification;
#[path = "support/environment_children.rs"]
mod environment_children;
mod environment_model_keys;
#[path = "support/invocation.rs"]
mod invocation;
#[path = "support/json_equivalence.rs"]
mod json_equivalence;
mod model_history_projection;
mod model_invocation;
mod model_records;
mod model_stream_lifetimes;
mod model_thinking_levels;
mod oauth_primitives;
mod offline_model_catalog;
#[path = "support/registry.rs"]
mod registry;
mod scripted_model_invocation;
mod simple_model_options;
mod stream_options;
mod streamed_arguments;
