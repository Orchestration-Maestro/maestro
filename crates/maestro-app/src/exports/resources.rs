//! Existing resources owners exposed at the application root.

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_resources::{SourceOrigin, SourceScope};

pub use maestro_resources::{
    ResourceEntry, ResourceFileType, canonicalize_path, current_directory, drive_directories,
};
