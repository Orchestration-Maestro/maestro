//! Shared integration-test executable.

mod browser_loop;
mod browser_owned;
mod continuations;
mod hook_boundaries;
mod live_subscriptions;
mod loop_turns;
mod maestro_agent_guide;
mod maestro_agent_integration;
mod maestro_proxy_stream;
mod message_projection;
mod owned_runs;
mod parallel_tools;
mod queued_turns;
mod raw_loop_cases;
mod raw_stream_contract;
mod run_binding;
mod run_configuration;
mod schema_argument_routes;
mod state_ownership;
#[cfg(not(target_arch = "wasm32"))]
mod support;
mod tool_argument_roots;
mod tool_callbacks;
mod tool_detail_presence;
mod tool_record_identity;
mod tool_turns;
mod turn_controls;

mod execution_affinity;
mod message_storage;

mod execution_replay;

mod provider_affinity;

mod executable_schemas;
