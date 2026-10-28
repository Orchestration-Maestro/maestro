//! Daemon integration tests compiled as one program.
#![cfg(test)]

#[cfg(target_os = "linux")]
mod config_support;
mod daemon_actions;
mod daemon_capture;
mod daemon_client;
mod daemon_config;
mod daemon_config_save;
mod daemon_construction;
mod daemon_environment;
mod daemon_events;
mod daemon_git_status;
mod daemon_ids;
mod daemon_instances;
mod daemon_paths;
mod daemon_runtime;
mod daemon_session;
mod daemon_state;
mod desktop_notification;
mod event_replay;
mod git_status;
mod key_combo;
mod repository_discovery;
mod repository_identity;
mod repository_support;
mod settings_routes;
#[cfg(target_os = "linux")]
mod support;
mod tab_status;
mod terminal_titles;
mod window_title;
mod workspace_aggregate;
