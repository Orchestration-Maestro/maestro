//! Tests that drive the daemon state through its own methods and read its fields.

#[cfg(target_os = "linux")]
mod config_support;
#[cfg(target_os = "linux")]
mod daemon_actions;
#[cfg(target_os = "linux")]
mod daemon_base_settings;
#[cfg(target_os = "linux")]
mod daemon_base_settings_save;
#[cfg(target_os = "linux")]
mod daemon_capture;
#[cfg(target_os = "linux")]
mod daemon_config;
#[cfg(target_os = "linux")]
mod daemon_config_save;
#[cfg(target_os = "linux")]
mod daemon_construction;
#[cfg(target_os = "linux")]
mod daemon_events;
#[cfg(target_os = "linux")]
mod daemon_git_status;
#[cfg(target_os = "linux")]
mod daemon_home;
#[cfg(target_os = "linux")]
mod daemon_ids;
#[cfg(target_os = "linux")]
mod daemon_runtime;
#[cfg(target_os = "linux")]
mod daemon_session;
#[cfg(target_os = "linux")]
mod daemon_state;
#[cfg(target_os = "linux")]
mod support;
#[cfg(target_os = "linux")]
mod tab_status;
#[cfg(target_os = "linux")]
mod terminal_titles;
#[cfg(target_os = "linux")]
mod window_title;
