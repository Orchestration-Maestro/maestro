//! The read-only footer metadata that extension footer factories receive.
//!
//! The application owns the metadata and its Git, watch, count and status policy;
//! this module declares what a footer may read and the live status view.

pub mod live;

use std::rc::Rc;

/// Extension status texts by key, in insertion order; clones share one
/// collection, so a retained view observes later changes.
pub type ExtensionStatuses = live::Live<String, String>;
/// Live iterator over [`ExtensionStatuses`].
pub type ExtensionStatusIter = live::LiveIter<String, String>;

/// What frontends and extensions may read of the footer metadata.
pub trait ReadonlyFooterDataProvider {
    /// The current branch: `None` outside a repository, `detached` for a
    /// detached HEAD. Reads are cached until a refresh or working-directory change.
    fn get_git_branch(&self) -> Option<String>;
    /// The extension status texts; the result stays live.
    fn get_extension_statuses(&self) -> ExtensionStatuses;
    /// The supplied count of providers with available models.
    fn get_available_provider_count(&self) -> f64;
    /// Call `callback` on working-directory changes and changed cached branches.
    ///
    /// Registering the same `Rc` twice keeps one registration. The returned
    /// function removes that registration; dropping it does not. Holding the
    /// function retains the watch lifecycle.
    fn on_branch_change(&self, callback: Rc<dyn Fn()>) -> Box<dyn Fn()>;
}
