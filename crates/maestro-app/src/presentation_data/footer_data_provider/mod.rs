//! Footer metadata: the cached Git branch, extension status texts and the
//! supplied available-provider count.
//!
//! [`FooterDataProvider`] owns the metadata. Frontends read it through
//! [`ReadonlyFooterDataProvider`]. File and process effects go through
//! [`FooterOperations`]; repository discovery uses the native workspace owner.
//! Paths are authored strings; their lexical rules belong to `maestro_path`.

mod git;
#[cfg(unix)]
mod native;
mod operations;
mod provider;
mod watch;

pub use maestro_extensions::footer_data_provider::{
    ExtensionStatusIter, ExtensionStatuses, ReadonlyFooterDataProvider,
};
#[cfg(unix)]
pub use native::NativeFooterOperations;
pub use operations::{FooterFileStats, FooterOperations};
pub use provider::FooterDataProvider;
