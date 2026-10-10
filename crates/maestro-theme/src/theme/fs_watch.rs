//! Theme aliases for the shared filesystem-watch effects.
#[cfg(not(target_arch = "wasm32"))]
pub use maestro_watch::fs_watch::NativeWatchOperations as NativeThemeWatchOperations;
pub use maestro_watch::fs_watch::{
    FsWatcher as ThemeWatcher, WatchOperations as ThemeWatchOperations,
    WatchTimer as ThemeReloadTimer, close_watcher, watch_with_error_handler,
};
