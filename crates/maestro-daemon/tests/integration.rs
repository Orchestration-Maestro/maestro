//! Daemon integration tests compiled as one program.
#![cfg(test)]

mod daemon_client;
mod daemon_environment;
mod daemon_instances;
mod daemon_paths;
mod desktop_notification;
mod event_replay;
mod git_status;
mod git_worktree;
mod key_combo;
mod local_sockets;
mod pane_chrome;
mod repository_discovery;
mod repository_identity;
mod repository_support;
mod settings_routes;
mod socket_paths;
mod workspace_aggregate;
