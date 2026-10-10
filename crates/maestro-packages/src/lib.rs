//! Package sources: identities, configuration, acquisition and removal.
#![doc = include_str!("../../../docs/package-sources.md")]

mod git;

pub use git::{GitSource, parse_git_url};

/// Trims the whitespace accepted in authored source and command text.
fn trim(text: &str) -> &str {
    text.trim_matches(|c: char| c == '\u{feff}' || (c.is_whitespace() && c != '\u{85}'))
}

mod package_manager;
#[cfg(not(target_arch = "wasm32"))]
pub use package_manager::NativePackageOperations;
pub use package_manager::{
    CommandOutput, ConfiguredPackage, DefaultPackageManager, InstalledSourceScope, PackageFuture,
    PackageManager, PackageManagerOptions, PackageOperations, ProgressAction, ProgressCallback,
    ProgressEvent, ProgressEventType,
};
