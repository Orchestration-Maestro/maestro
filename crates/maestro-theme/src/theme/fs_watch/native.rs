//! Native watch and timer adapter over operating-system notifications.
use super::{ThemeReloadTimer, ThemeWatchOperations, ThemeWatcher};
use notify::event::EventKind;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::{cell::Cell, io, path::Path, rc::Rc, time::Duration};
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
        let (results, mut rx) = unbounded_channel();
        let sender = results.clone();
        let notifier = notify::recommended_watcher(move |result| {
            let _ = sender.send(result);
        })
        .and_then(|mut notifier| {
            notifier.watch(&root, RecursiveMode::NonRecursive)?;
            Ok(notifier)
        })
        .map_err(io::Error::other)?;
        let closed = Rc::new(Cell::new(false));
        let stopped = Rc::clone(&closed);
        let task = self.local.spawn_local(async move {
            while let Some(result) = rx.recv().await {
                match result {
                    Ok(event) => entry_names(&event, &root)
                        .into_iter()
                        .take_while(|_| !stopped.get())
                        .for_each(|name| listener(name)),
                    Err(_) if !stopped.get() => on_error(),
                    Err(_) => (),
                }
            }
        });
        let watcher = NativeWatcher {
            notifier: Some(notifier),
            task: Aborting(task),
            closed,
        };
        Ok((watcher, results))
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
        .map(|path| {
            let relative = path.strip_prefix(dir).ok()?;
            (!relative.as_os_str().is_empty()).then(|| relative.to_string_lossy().into_owned())
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
    notifier: Option<RecommendedWatcher>,
    /// Task that dispatches notifications.
    task: Aborting,
    /// Checked before every dispatch, because aborting cannot interrupt a running callback.
    closed: Rc<Cell<bool>>,
}

impl ThemeWatcher for NativeWatcher {
    fn close(&mut self) -> io::Result<()> {
        self.closed.set(true);
        self.notifier = None;
        self.task.cancel();
        Ok(())
    }
}

impl ThemeReloadTimer for Aborting {
    fn cancel(&mut self) {
        self.0.abort();
    }
}
