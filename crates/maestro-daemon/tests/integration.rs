//! Daemon integration tests compiled as one program.
#![cfg(test)]

mod daemon_api;
mod daemon_client;
mod daemon_environment;
mod daemon_events;
mod daemon_instances;
mod daemon_layout;
mod daemon_paths;
mod desktop_notification;
mod event_replay;
mod git_status;
mod git_worktree;
mod key_combo;
mod layout_apply;
mod layout_commands;
mod local_sockets;
mod pane_chrome;
mod pane_close;
mod pane_commands;
mod pane_input;
mod pane_metadata;
mod pane_move;
mod pane_read;
mod pane_rename;
mod pane_reports;
mod pane_resize;
mod pane_split;
mod pane_swap;
mod pane_zoom;
mod repository_discovery;
mod repository_identity;
mod repository_support;
mod settings_routes;
mod socket_paths;
#[cfg(target_os = "linux")]
mod support;
mod tab_commands;
mod workspace_aggregate;
mod workspace_commands;
