//! Existing packages owners exposed at the application root.

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_packages::{
    ConfiguredPackage, DefaultPackageManager, InstalledSourceScope, NativePackageOperations,
    PackageFuture, PackageManager, PackageManagerOptions, PackageOperations, PathMetadata,
    ProgressAction, ProgressCallback, ProgressEvent, ProgressEventType, ResolvedPaths,
    ResolvedResource,
};
