//! Existing packages owners exposed at the application root.

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_packages::{
    DefaultPackageManager, PackageManager, PathMetadata, ProgressCallback, ProgressEvent,
    ResolvedPaths, ResolvedResource,
};
