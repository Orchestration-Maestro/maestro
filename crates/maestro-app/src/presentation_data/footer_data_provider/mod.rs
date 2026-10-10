//! Footer metadata: the cached Git branch, extension status texts and the
//! supplied available-provider count.
//!
//! [`FooterDataProvider`] owns the metadata. Frontends read it through
//! [`ReadonlyFooterDataProvider`]. File and process effects go through
//! [`FooterOperations`], so native and controlled adapters are interchangeable.
//! Paths are authored strings; their lexical rules belong to `maestro_path`.

mod git;
mod live;
#[cfg(unix)]
mod native;
mod operations;
mod provider;

#[cfg(unix)]
pub use native::NativeFooterOperations;
pub use operations::{FooterFileKind, FooterOperations};
pub use provider::{FooterDataProvider, ReadonlyFooterDataProvider};

/// Extension status texts by key, in insertion order; clones share one
/// collection, so a retained view observes later changes.
pub type ExtensionStatuses = live::Live<String, String>;
/// Live iterator over [`ExtensionStatuses`].
pub type ExtensionStatusIter = live::LiveIter<String, String>;
