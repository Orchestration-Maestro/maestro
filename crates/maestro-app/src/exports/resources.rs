//! Existing resources owners exposed at the application root.

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_resources::{NativeResourceOperations, SourceOrigin, SourceScope};

pub use maestro_resources::{
    ResourceEntry, ResourceFileType, ResourceOperations, canonicalize_path,
};
