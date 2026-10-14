//! Shared integration-test executable.

use chat::TestResult;
use maestro_models::{Model, ModelCompat, ModelCost, ModelInput, ModelThinkingLevel};

mod account_message_routing;
mod account_tool_names;
#[path = "support/azure_cases.rs"]
mod azure_cases;
#[path = "support/azure_endpoint.rs"]
mod azure_endpoint;
mod azure_endpoint_requests;
mod azure_endpoint_streams;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/bedrock.rs"]
mod bedrock;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/bedrock_cases.rs"]
mod bedrock_cases;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/bedrock_routes.rs"]
mod bedrock_routes;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/bedrock_rows.rs"]
mod bedrock_rows;
mod browser_build;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/bundled.rs"]
mod bundled;
mod bundled_dispatch;
mod bundled_options;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/cache_overflow_images.rs"]
mod cache_overflow_images;
mod cache_preferences;
#[path = "support/cases.rs"]
mod cases;
mod catalog_command;
mod catalog_generation;
mod catalog_route_smoke;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/chat.rs"]
mod chat;
mod chat_completion_events;
mod chat_completion_helpers;
mod chat_completion_requests;
mod chat_empty_tools;
#[path = "support/chat_qualification.rs"]
mod chat_qualification;
mod chat_response_identity;
#[path = "chat_tool_choices.rs"]
mod chat_tool_choices;
mod checked_tool_arguments;
#[path = "support/child_process.rs"]
mod child_process;
mod cloud_conversation_configuration;
mod cloud_conversation_conversion;
mod cloud_conversation_hook_content;
mod cloud_conversation_reduction;
mod cloud_conversation_registration;
mod cloud_conversation_streams;
mod cloud_conversation_unknown_unions;
mod cloud_model_routes;
mod cloud_profile_credentials;
mod cloud_proxy_transports;
mod compatibility_invocation;
mod content_generation_conversion;
mod content_generation_invocation;
mod content_generation_preparation;
mod content_generation_reduction;
mod content_generation_transport;
mod content_generation_wire;
#[path = "support/content_preparation.rs"]
mod content_preparation;
mod content_thinking_disabled;
#[path = "support/content_wire.rs"]
mod content_wire;
mod context_overflow_classification;
mod conversation_reasoning;
mod conversation_streams;
mod conversation_tool_schema;
mod device_account_authorization;
#[path = "empty_content.rs"]
mod empty_content;
#[path = "support/environment_children.rs"]
mod environment_children;
mod environment_model_keys;
#[path = "support/environment_rows.rs"]
mod environment_rows;
mod handoff_tool_ids;
#[path = "history_handoffs.rs"]
mod history_handoffs;
mod http_execution;
mod http_status_text;
mod interleaved_reasoning;
mod invocation_cancellation;
#[path = "support/json.rs"]
pub(crate) mod json;
mod json_boundary_consumers;
#[path = "support/key_selection.rs"]
mod key_selection;
#[allow(
    dead_code,
    reason = "Each test binary uses part of the loopback server."
)]
#[path = "support/loopback.rs"]
mod loopback;
mod message_adaptive_thinking;
mod message_eager_input;
mod message_long_retention;
#[path = "support/message_options.rs"]
mod message_options;
#[path = "support/message_probes.rs"]
mod message_probes;
mod message_protocol_options;
mod message_protocol_requests;
mod message_protocol_streams;
#[path = "support/message_qualification.rs"]
mod message_qualification;
mod message_thinking_disabled;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/messages.rs"]
mod messages;
mod missing_tool_results;
mod model_history_projection;
mod model_invocation;
mod model_records;
mod model_stream_lifetimes;
#[path = "model_streaming.rs"]
mod model_streaming;
mod model_thinking_levels;
mod oauth_primitives;
mod oauth_provider_registry;
mod offline_model_catalog;
#[cfg(unix)]
#[path = "support/profile_credentials.rs"]
mod profile_credentials;
#[cfg(unix)]
#[path = "support/profile_files.rs"]
mod profile_files;
#[cfg(unix)]
#[path = "support/profile_routes.rs"]
mod profile_routes;
#[cfg(unix)]
#[path = "support/profile_services.rs"]
mod profile_services;
#[path = "provider_overflow.rs"]
mod provider_overflow;
#[path = "provider_unicode.rs"]
mod provider_unicode;
mod reasoning_capabilities;
#[path = "support/reasoning_qualification.rs"]
mod reasoning_qualification;
mod response_account_authorization;
mod response_affinity;
#[path = "support/response_cases.rs"]
mod response_cases;
#[path = "support/response_endpoint.rs"]
mod response_endpoint;
mod response_endpoint_requests;
mod response_endpoint_streams;
mod response_identity;
#[path = "support/response_output.rs"]
mod response_output;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/response_qualification.rs"]
mod response_qualification;
mod response_reasoning_replay;
#[path = "support/response_runtime.rs"]
mod response_runtime;
mod response_session_affinity;
mod response_session_surface;
mod response_tool_images;
mod routed_cache_write;
mod schema_checker_qualification;
mod schema_diagnostics;
mod scripted_model_invocation;
mod simple_model_options;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/socks.rs"]
mod socks;
mod streamed_arguments;
mod subscription_accounts;
#[path = "tool_images.rs"]
mod tool_images;
#[allow(
    dead_code,
    reason = "Shared helpers serve multiple integration targets."
)]
#[path = "support/transport.rs"]
mod transport;
mod usage_after_abort;
#[path = "usage_totals/cases.rs"]
mod usage_cases;
mod usage_totals;
