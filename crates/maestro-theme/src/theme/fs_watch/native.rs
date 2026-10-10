//! Native watch and timer adapter over operating-system notifications.
use super::{ThemeReloadTimer, ThemeWatchOperations, ThemeWatcher};
use notify::event::EventKind;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::cell::Cell;
use std::io;
use std::path::Path;
use std::rc::Rc;
use std::time::Duration;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tokio::task::{JoinHandle, LocalSet};

/// Notifications and timers delivered on a caller-driven [`LocalSet`].
///
/// The caller owns the local set and must drive it, for example with
/// [`LocalSet::run_until`], inside a runtime whose timer is enabled. Closing or
/// dropping a native watch or timer handle cancels its pending work.
pub struct NativeThemeWatchOperations {
    /// Set that runs every dispatched callback.
    local: Rc<LocalSet>,
}

impl NativeThemeWatchOperations {
    /// Dispatch callbacks on `local`.
    #[must_use]
    pub fn new(local: Rc<LocalSet>) -> Self {
        Self { local }
    }

    /// Register a watch and return it with the channel its results enter.
    ///
    /// The directory is anchored to the working directory once, as the notifier
    /// anchors it, so event paths project to entry names.
    pub(super) fn open(
        &self,
        path: &str,
        listener: Rc<dyn Fn(Option<String>)>,
        on_error: Rc<dyn Fn()>,
    ) -> io::Result<(NativeWatcher, UnboundedSender<notify::Result<Event>>)> {
        let root = std::path::absolute(path)?;
        let (tx, mut rx) = unbounded_channel();
        let sender = tx.clone();
        let mut watcher = notify::recommended_watcher(move |result| {
            let _ = sender.send(result);
        })
        .map_err(io::Error::other)?;
        watcher
            .watch(&root, RecursiveMode::NonRecursive)
            .map_err(io::Error::other)?;
        let closed = Rc::new(Cell::new(false));
        let stopped = Rc::clone(&closed);
        let task = self.local.spawn_local(async move {
            while let Some(result) = rx.recv().await {
                dispatch(result, &root, &stopped, &*listener, &*on_error);
            }
        });
        let watcher = NativeWatcher {
            watcher: Some(watcher),
            task: Aborting(task),
            closed,
        };
        Ok((watcher, tx))
    }
}

impl ThemeWatchOperations for NativeThemeWatchOperations {
    fn watch(
        &self,
        path: &str,
        listener: Rc<dyn Fn(Option<String>)>,
        on_error: Rc<dyn Fn()>,
    ) -> io::Result<Box<dyn ThemeWatcher>> {
        let (watcher, _) = self.open(path, listener, on_error)?;
        Ok(Box::new(watcher))
    }

    fn schedule(&self, delay: Duration, callback: Box<dyn FnOnce()>) -> Box<dyn ThemeReloadTimer> {
        let task = self.local.spawn_local(async move {
            tokio::time::sleep(delay).await;
            callback();
        });
        Box::new(Aborting(task))
    }
}

/// Deliver one notifier result unless the watch was closed, even by an earlier name.
fn dispatch(
    result: notify::Result<Event>,
    root: &Path,
    stopped: &Cell<bool>,
    listener: &dyn Fn(Option<String>),
    on_error: &dyn Fn(),
) {
    match result {
        Ok(event) => {
            for name in entry_names(&event, root) {
                if stopped.get() {
                    return;
                }
                listener(name);
            }
        }
        Err(_) if !stopped.get() => on_error(),
        Err(_) => {}
    }
}

/// Entry names an event changed, excluding reads; `None` marks an unknown name.
pub(super) fn entry_names(event: &Event, dir: &Path) -> Vec<Option<String>> {
    if matches!(event.kind, EventKind::Access(_)) {
        return Vec::new();
    }
    if event.need_rescan() || event.paths.is_empty() {
        return vec![None];
    }
    event
        .paths
        .iter()
        .map(|path| match path.strip_prefix(dir) {
            Ok(relative) if !relative.as_os_str().is_empty() => {
                Some(relative.to_string_lossy().into_owned())
            }
            _ => None,
        })
        .collect()
}

/// A native timer or receiver task that is aborted when dropped.
struct Aborting(JoinHandle<()>);

impl Drop for Aborting {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// A native watch; once closed, no further notification is dispatched.
pub(super) struct NativeWatcher {
    /// Library watch; dropping it stops the notifier.
    watcher: Option<RecommendedWatcher>,
    /// Task that dispatches notifications.
    task: Aborting,
    /// Checked before every dispatch, because aborting cannot interrupt a running callback.
    closed: Rc<Cell<bool>>,
}

impl ThemeWatcher for NativeWatcher {
    fn close(&mut self) -> io::Result<()> {
        self.closed.set(true);
        self.watcher = None;
        self.task.0.abort();
        Ok(())
    }
}

impl ThemeReloadTimer for Aborting {
    fn cancel(&mut self) {
        self.0.abort();
    }
}
