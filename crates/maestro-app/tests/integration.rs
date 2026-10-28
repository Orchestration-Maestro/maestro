//! Shared integration-test executable.
#![cfg(test)]

mod agent_session_branching_checks;
mod agent_session_concurrent_checks;
mod agent_session_model_extension_checks;
mod agent_session_prompt_checks;
mod agent_session_queue_checks;
mod agent_session_retry_checks;
mod agent_session_retry_events_checks;
mod agent_session_runtime_checks;
mod agent_session_runtime_events_checks;
mod agent_session_stats_checks;
mod agent_session_tree_navigation_checks;
mod analysis_cost;
#[cfg(not(target_arch = "wasm32"))]
#[path = "support/analysis_fixtures.rs"]
mod analysis_fixtures;
mod analysis_shared;
mod analysis_tools;
mod analysis_transcripts;
mod analysis_usage;
mod application_keybindings;
mod artifact_runtime_access;
mod attachment_loading;
mod base_package_loading;
#[path = "docx_support/body_tree.rs"]
mod body_tree;
mod browser_contributions;
mod browser_decimal;
mod browser_discovery;
mod browser_messages;
mod browser_proxy;
mod browser_schema_origins;
mod browser_sessions;
mod browser_storage;
#[path = "support/browser_storage.rs"]
mod browser_storage_support;
mod changelog;
#[cfg(unix)]
#[path = "support/child.rs"]
mod child;
mod clipboard_copy;
mod clipboard_images;
#[cfg(not(target_arch = "wasm32"))]
#[path = "support/clipboard_platform.rs"]
mod clipboard_platform;
#[cfg(not(target_arch = "wasm32"))]
#[path = "support/clipboard_status.rs"]
mod clipboard_status;
#[cfg(not(target_arch = "wasm32"))]
#[path = "support/command_operations.rs"]
mod command_operations;
mod configuration_paths;
mod context_discovery;
mod daemon_connection;
#[cfg(unix)]
#[path = "support/default_configuration.rs"]
mod default_configuration;
mod default_path_routes;
mod diff_root_exports;
mod document_extraction;
mod docx_attachments;
mod docx_body;
mod docx_comments;
mod docx_drawing_records;
mod docx_formatting;
mod docx_metadata;
mod docx_metadata_packages;
mod docx_numbering_fonts;
mod docx_packages;
mod docx_paragraph_properties;
mod docx_parts;
mod docx_properties;
mod docx_section_properties;
mod docx_support;
mod docx_tables;
mod docx_values;
mod docx_xml;
mod extension_context_exports;
mod extension_ui;
mod extension_ui_exports;
#[path = "docx_support/font_resources.rs"]
mod font_resources;
mod footer_cwd_identity;
mod footer_metadata;
#[path = "support/mod.rs"]
mod footer_support;
mod footer_watch;
mod helper_launches;
mod hold_unchecked_text_checks;
mod install_reporting;
mod installation_kind;
#[cfg(not(target_arch = "wasm32"))]
#[path = "support/isolated_home.rs"]
mod isolated_home;
mod javascript_execution;
mod legacy_workbook_support;
mod legacy_workbooks;
mod lm_studio_discovery;
#[cfg(not(target_arch = "wasm32"))]
mod lm_studio_support;
mod message_end_cost_override_checks;
#[path = "docx_support/metadata.rs"]
mod metadata;
mod model_selector_exports;
#[cfg(unix)]
#[path = "support/native.rs"]
mod native;
mod network_connection_lost_retry_checks;
#[path = "docx_support/numbering.rs"]
mod numbering_observation;
mod output_routing;
mod package_command_execution;
#[cfg(not(target_arch = "wasm32"))]
#[path = "support/package_fixtures.rs"]
mod package_fixtures;
mod package_manager_checks;
mod pdf_attachments;
mod pdf_pages;
mod pptx_attachments;
mod prompt_templates_checks;
mod queued_slash_command_followup_checks;
mod reload_stale_resource_settings_checks;
mod replaced_session_context_checks;
mod resource_extension_loading;
#[cfg(unix)]
#[path = "support/resource_fixtures.rs"]
mod resource_fixtures;
mod resource_loading;
#[cfg(unix)]
#[path = "../examples/rpc_client_demo/interactive.rs"]
mod rpc_client_demo;
mod rpc_client_events;
mod rpc_client_lifecycle;
mod rpc_client_transport;
#[cfg(unix)]
#[path = "support/rpc_peer.rs"]
mod rpc_peer;
mod runtime_agent;
mod sandbox_runtime_providers;
mod sdk_root_exports;
mod self_update_native;
mod self_update_selection;
mod server_commands;
mod server_events;
mod server_lifetime;
#[cfg(unix)]
#[path = "support/service_observations.rs"]
mod service_observations;
mod service_releases;
mod session_account_file;
mod session_bash;
mod session_constructor;
mod session_events;
mod session_exports;
mod session_facade;
mod session_harness_checks;
mod session_messages;
mod session_model_selection;
mod session_name_event_checks;
mod session_prompt_preflight;
mod session_queues;
mod session_runtime;
mod session_sdk;
mod session_selection_ownership;
mod session_services;
mod session_skill_blocks;
mod session_statistics;
mod session_tree_navigation;
mod settings_callback_exports;
mod settings_manager_bug_checks;
mod settings_manager_checks;
mod share_links;
mod shell_root_exports;
mod shipped_examples;
mod skill_precedence;
mod spreadsheet_attachments;
mod spreadsheet_html;
mod spreadsheet_support;
mod ssf_detection;
mod ssf_format;
mod ssf_support;
mod ssf_values;
mod ssf_writers;
mod startup_completion;
mod startup_conversions;
mod startup_environment;
mod startup_timings;
mod system_prompt_assembly;
/// Strict recorded observations shared by several test modules.
mod support {
    mod runtime;
    #[cfg(not(target_arch = "wasm32"))]
    pub use runtime::*;
    pub mod attachment_observation;
    pub mod attachments;
    pub mod browser_records;
    pub mod browser_storage_records;
    #[cfg(not(target_arch = "wasm32"))]
    pub mod document_attachments;
    #[cfg(not(target_arch = "wasm32"))]
    pub mod document_extraction;
    #[cfg(not(target_arch = "wasm32"))]
    pub mod export_fixtures;
    #[cfg(not(target_arch = "wasm32"))]
    pub mod javascript_execution;
    pub mod no_commands;
    #[cfg(not(target_arch = "wasm32"))]
    pub mod pdf;
    #[cfg(not(target_arch = "wasm32"))]
    pub mod rpc_connection;
    #[cfg(not(target_arch = "wasm32"))]
    pub mod runtime_fixtures;
    pub mod session_harness;
    #[cfg(not(target_arch = "wasm32"))]
    mod shared_operands;
    pub mod spreadsheet;
    #[cfg(not(target_arch = "wasm32"))]
    pub mod spreadsheet_native;
}
mod theme_selection;
mod tree_cancel_compacting_checks;
mod tree_interface;
mod tree_traversal_checks;
#[cfg(not(target_arch = "wasm32"))]
#[path = "support/watching.rs"]
mod watching;
mod wire_records;
mod workbook_fixture;
mod workbooks;

#[cfg(not(target_arch = "wasm32"))]
#[path = "support/artifact_presentation.rs"]
mod artifact_presentation_support;

mod executable_schemas;

mod session_decisions;
