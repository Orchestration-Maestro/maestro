//! Replaceable directory-watch and timer effects, with safe creation and close helpers.
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests;

#[cfg(not(target_arch = "wasm32"))]
pub use native::NativeWatchOperations;
use std::io;
use std::rc::Rc;
use std::time::Duration;

/// Default retry delay in milliseconds; scheduling belongs to the caller.
pub const FS_WATCH_RETRY_DELAY_MS: u64 = 5000;

/// An active directory watch.
pub trait FsWatcher {
    /// Stop delivering notifications.
    ///
    /// # Errors
    /// Returns the adapter's failure to release the watch.
    fn close(&mut self) -> io::Result<()>;
}

/// A scheduled one-shot callback.
pub trait WatchTimer {
    /// Prevent the callback from running if it has not run yet.
    fn cancel(&mut self);
}

/// Replaceable directory-watch and timer effects.
pub trait WatchOperations {
    /// Watch the entries of one directory without descending into subdirectories.
    ///
    /// `listener` receives each changed entry name, or `None` when the name is
    /// unknown; `on_error` is called when the watch fails. Neither is called before
    /// this method returns.
    ///
    /// # Errors
    /// Returns the adapter's failure to create the watch.
    fn watch(
        &self,
        path: &str,
        listener: Rc<dyn Fn(Option<String>)>,
        on_error: Rc<dyn Fn()>,
    ) -> io::Result<Box<dyn FsWatcher>>;
    /// Run `callback` once after `delay`, never before this method returns.
    fn schedule(&self, delay: Duration, callback: Box<dyn FnOnce()>) -> Box<dyn WatchTimer>;
}

/// Close a watch if there is one, ignoring a close failure.
pub fn close_watcher(watcher: Option<Box<dyn FsWatcher>>) {
    if let Some(mut watcher) = watcher {
        let _ = watcher.close();
    }
}

/// Create a watch, reporting a creation failure through `on_error` as absence.
#[must_use]
pub fn watch_with_error_handler(
    path: &str,
    listener: Rc<dyn Fn(Option<String>)>,
    on_error: Rc<dyn Fn()>,
    operations: &dyn WatchOperations,
) -> Option<Box<dyn FsWatcher>> {
    operations
        .watch(path, listener, Rc::clone(&on_error))
        .map_err(move |_| on_error())
        .ok()
}
