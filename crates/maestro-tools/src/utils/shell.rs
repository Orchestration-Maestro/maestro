//! Shell discovery, detached-child cleanup and captured-output display cleaning.

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::{
    ShellConfig, ShellEnv, get_shell_config, get_shell_env, kill_process_tree,
    kill_tracked_detached_children, track_detached_child_pid, untrack_detached_child_pid,
};

/// Drop C0 controls other than TAB, LF and CR, and the interlinear annotation marks
/// U+FFF9 to U+FFFB; DEL, C1 controls, ZWJ and every other scalar stay.
#[must_use]
pub fn sanitize_binary_output(text: &str) -> String {
    text.chars()
        .filter(|&c| {
            matches!(c, '\t' | '\n' | '\r')
                || !(c <= '\u{1f}' || ('\u{fff9}'..='\u{fffb}').contains(&c))
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod tests;
