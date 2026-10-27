//! Package sources: identities, configuration, acquisition and removal.
#![doc = include_str!("../../../docs/package-sources.md")]

mod git;

pub use git::{GitSource, parse_git_url};

/// Trims the whitespace accepted in authored source and command text.
fn trim(text: &str) -> &str {
    text.trim_matches(whitespace)
}

use maestro_text::is_whitespace_scalar as whitespace;

mod package_manager;
#[cfg(not(target_arch = "wasm32"))]
pub use package_manager::NativePackageOperations;
pub use package_manager::{
    CommandCaptureOptions, CommandOutput, ConfiguredPackage, DefaultPackageManager, GitPinStatus,
    GitRelease, InstalledSourceScope, MissingSourceAction, PackageFuture, PackageManager,
    PackageManagerOptions, PackageOperations, PackageUpdate, PackageUpdateType, ProgressAction,
    ProgressCallback, ProgressEvent, ProgressEventType, ResolvedPackage, ResolvedPaths,
    ResolvedResource,
};

pub use maestro_pattern::{PatternError, minimatch};

pub use maestro_resources::{PathMetadata, SourceOrigin, SourceScope};
