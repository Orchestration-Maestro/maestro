//! Existing packages owners exposed at the application root.

#[cfg(not(target_arch = "wasm32"))]
pub use maestro_packages::{
    CommandCaptureOptions, CommandOutput, ConfiguredPackage, DefaultPackageManager, GitSource,
    InstalledSourceScope, NativePackageOperations, PackageFuture, PackageManager,
    PackageManagerOptions, PackageOperations, PathMetadata, ProgressAction, ProgressCallback,
    ProgressEvent, ProgressEventType, ResolvedPaths, ResolvedResource, parse_git_url,
};
