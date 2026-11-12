//! Shell discovery, detached-child cleanup and captured-output display cleaning.

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::{
    ShellConfig, ShellEnv, get_shell_config, get_shell_env, kill_process_tree,
    kill_tracked_detached_children, track_detached_child_pid, untrack_detached_child_pid,
};

pub use maestro_text::terminal::sanitize_binary_output;

#[cfg(test)]
pub(crate) mod tests;
