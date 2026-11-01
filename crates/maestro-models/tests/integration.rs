//! Shared integration-test executable.

#[cfg(not(target_arch = "wasm32"))]
use chat::TestResult;
#[cfg(not(target_arch = "wasm32"))]
use maestro_models::{Model, ModelCompat, ModelCost, ModelInput, ModelThinkingLevel};

mod account_message_routing;
mod account_tool_names;
mod assistant_publication;
mod authored_tool_arguments;
#[path = "support/azure_cases.rs"]
mod azure_cases;
#[path = "support/azure_endpoint.rs"]
mod azure_endpoint;
mod azure_endpoint_requests;
mod azure_endpoint_streams;
#[path = "support/bedrock.rs"]
mod bedrock;
#[path = "support/bedrock_cases.rs"]
mod bedrock_cases;
#[path = "support/bedrock_routes.rs"]
mod bedrock_routes;
#[path = "support/bedrock_rows.rs"]
mod bedrock_rows;
#[cfg(target_os = "linux")]
#[path = "support/bedrock_tls_routes.rs"]
mod bedrock_tls_routes;
mod browser_build;
#[path = "support/bundled.rs"]
mod bundled;
mod bundled_dispatch;
mod bundled_options;
#[path = "support/cache_overflow_images.rs"]
mod cache_overflow_images;
mod cache_preferences;
#[path = "support/cases.rs"]
mod cases;
mod catalog_command;
#[path = "support/catalog_command.rs"]
mod catalog_command_support;
mod catalog_generation;
mod catalog_route_smoke;
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
mod circle_fixture;
#[path = "support/clock.rs"]
mod clock;
mod cloud_content_projection;
mod cloud_conversation_configuration;
mod cloud_conversation_conversion;
mod cloud_conversation_hook_content;
mod cloud_conversation_reduction;
mod cloud_conversation_registration;
mod cloud_conversation_runtime;
mod cloud_conversation_streams;
mod cloud_conversation_unknown_unions;
mod cloud_model_routes;
mod cloud_pac_routing;
mod cloud_profile_credentials;
mod cloud_proxy_transports;
mod cloudflare_dispatch;
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
mod executable_schemas;
#[allow(
    dead_code,
    reason = "Each test binary uses part of the scripted FTP peer."
)]
#[path = "support/ftp_server.rs"]
mod ftp_server;
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
#[path = "support/json_equivalence.rs"]
mod json_equivalence;
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
#[path = "support/messages.rs"]
mod messages;
mod missing_tool_results;
mod model_guide_examples;
mod model_history_projection;
mod model_invocation;
mod model_records;
mod model_stream_lifetimes;
#[path = "model_streaming.rs"]
mod model_streaming;
mod model_thinking_levels;
#[path = "support/native_peer.rs"]
mod native_peer;
#[path = "support/oauth_admission.rs"]
mod oauth_admission;
mod oauth_primitives;
mod oauth_provider_registry;
mod offline_model_catalog;
mod output_budget;
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
mod provider_affinity;
#[path = "provider_overflow.rs"]
mod provider_overflow;
#[path = "provider_unicode.rs"]
mod provider_unicode;
mod reasoning_capabilities;
#[path = "support/reasoning_qualification.rs"]
mod reasoning_qualification;
mod response_account_authorization;
mod response_affinity;
#[path = "support/response_case_schema.rs"]
mod response_case_schema;
#[path = "support/response_cases.rs"]
mod response_cases;
#[path = "support/response_endpoint.rs"]
mod response_endpoint;
mod response_endpoint_requests;
mod response_endpoint_streams;
mod response_identity;
#[path = "support/response_output.rs"]
mod response_output;
#[path = "support/response_qualification.rs"]
mod response_qualification;
mod response_reasoning_replay;
#[path = "support/response_runtime.rs"]
mod response_runtime;
mod response_session_affinity;
mod response_session_surface;
#[path = "support/response_support.rs"]
mod response_support;
mod response_tool_images;
#[path = "support/root_routes.rs"]
mod root_routes;
mod routed_cache_write;
#[path = "support/runtime.rs"]
mod runtime;
mod schema_checker_qualification;
mod schema_diagnostics;
mod scripted_model_invocation;
mod simple_model_options;
#[path = "support/socks.rs"]
mod socks;
mod stream_options;
mod streamed_arguments;
mod subscription_accounts;
#[cfg(target_os = "linux")]
#[path = "support/tls.rs"]
mod tls;
mod tool_argument_roots;
#[path = "tool_images.rs"]
mod tool_images;
#[path = "support/transport.rs"]
mod transport;
mod usage_after_abort;
#[path = "usage_totals/cases.rs"]
mod usage_cases;
mod usage_totals;

mod cloud_content_invocation;

#[path = "support/content_credentials.rs"]
mod content_credentials;

mod message_beta_headers;

mod simple_missing_auth;
