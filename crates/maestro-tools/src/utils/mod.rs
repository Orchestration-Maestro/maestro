//! Image preparation utilities.

#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod diagnostics;
pub(crate) mod shell;
pub use shell::sanitize_binary_output;
#[cfg(not(target_arch = "wasm32"))]
pub use shell::{
    ShellConfig, ShellEnv, get_shell_config, get_shell_env, kill_process_tree,
    kill_tracked_detached_children, track_detached_child_pid, untrack_detached_child_pid,
};
pub(crate) mod exif_orientation;
pub mod image_convert;
pub use exif_orientation::apply_exif_orientation;
pub(crate) mod image_resize;
pub use image_resize::{ImageResizeOptions, ResizedImage, format_dimension_note, resize_image};
pub mod mime;
