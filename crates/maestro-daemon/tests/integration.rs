//! Daemon integration tests compiled as one program.
#![cfg(test)]

#[cfg(target_os = "linux")]
mod config_support;
mod daemon_actions;
mod daemon_api;
mod daemon_capture;
mod daemon_client;
mod daemon_config;
mod daemon_config_save;
mod daemon_construction;
mod daemon_environment;
mod daemon_events;
mod daemon_ids;
mod daemon_instances;
mod daemon_layout;
mod daemon_paths;
mod daemon_runtime;
mod daemon_session;
mod daemon_state;
mod desktop_notification;
mod event_replay;
mod key_combo;
mod pane_chrome;
mod pane_close;
mod pane_commands;
mod pane_move;
mod pane_resize;
mod pane_split;
mod pane_swap;
mod pane_zoom;
mod repository_discovery;
mod repository_identity;
mod repository_support;
mod settings_routes;
#[cfg(target_os = "linux")]
mod support;
mod tab_commands;
mod tab_status;
mod terminal_titles;
mod window_title;
mod workspace_aggregate;
mod workspace_commands;
