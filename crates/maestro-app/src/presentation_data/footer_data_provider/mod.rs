//! Footer metadata: the cached Git branch, extension status texts and the
//! supplied available-provider count.
//!
//! [`FooterDataProvider`] owns the metadata. Frontends read it through
//! [`ReadonlyFooterDataProvider`]. Watch registration, timer scheduling, polling and Git
//! process effects go through [`FooterOperations`]; repository discovery, the `HEAD` read
//! and the presence checks read the file system directly, and paths stay authored strings
//! whose lexical rules belong to `maestro_path`.

mod git;
#[cfg(any(unix, windows))]
mod native;
mod operations;
mod provider;
mod watch;

pub use maestro_extensions::footer_data_provider::{
    ExtensionStatusIter, ExtensionStatuses, ReadonlyFooterDataProvider,
};
pub use maestro_watch::fs_watch::{FsWatcher, WatchOperations, WatchTimer};
#[cfg(any(unix, windows))]
pub use native::NativeFooterOperations;
pub use operations::{FooterFileStats, FooterOperations};
pub use provider::FooterDataProvider;
