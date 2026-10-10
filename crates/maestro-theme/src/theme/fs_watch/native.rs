//! Native watch and timer adapter over operating-system notifications.
use super::{ThemeReloadTimer, ThemeWatchOperations, ThemeWatcher};
use notify::event::EventKind;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tokio::task::{JoinHandle, LocalSet};

/// Notifications and timers delivered on a caller-driven [`LocalSet`].
///
/// The caller owns the local set and must drive it, for example with
/// [`LocalSet::run_until`], inside a runtime whose timer is enabled. Dropping a
/// handle or the last operations value cancels its pending work.
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

    /// Register a watch and return it with the sink its notifications enter.
    pub(super) fn open(
        &self,
        path: &str,
        listener: Rc<dyn Fn(Option<String>)>,
        on_error: Rc<dyn Fn()>,
    ) -> io::Result<(NativeWatcher, Sink)> {
        let (tx, mut rx) = unbounded_channel();
        let sink = Sink {
            tx,
            dir: PathBuf::from(path),
        };
        let handler = sink.clone();
        let mut watcher = notify::recommended_watcher(move |result| handler.deliver(result))
            .map_err(io::Error::other)?;
        watcher
            .watch(Path::new(path), RecursiveMode::NonRecursive)
            .map_err(io::Error::other)?;
        let task = self.local.spawn_local(async move {
            while let Some(message) = rx.recv().await {
                match message {
                    Message::Changed(name) => listener(name),
                    Message::Failed => on_error(),
                }
            }
        });
        Ok((
            NativeWatcher {
                watcher: Some(watcher),
                task,
            },
            sink,
        ))
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
        Box::new(NativeTimer(task))
    }
}

/// One notification crossing from the notifier's thread.
enum Message {
    /// An entry changed; `None` when its name is unknown.
    Changed(Option<String>),
    /// The watch failed.
    Failed,
}

/// Channel end that carries projected notifications off the notifier's thread.
#[derive(Clone)]
pub(super) struct Sink {
    /// Notifications in arrival order.
    tx: UnboundedSender<Message>,
    /// Watched directory.
    dir: PathBuf,
}

impl Sink {
    /// Project one library result and send each entry name or the failure.
    ///
    /// A path outside the directory, or the directory itself, has no entry name.
    pub(super) fn deliver(&self, result: notify::Result<Event>) {
        match result {
            Ok(event) => {
                for name in entry_names(&event, &self.dir) {
                    let _ = self.tx.send(Message::Changed(name));
                }
            }
            Err(_) => {
                let _ = self.tx.send(Message::Failed);
            }
        }
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

/// A native watch whose receiver task is aborted when it closes or drops.
pub(super) struct NativeWatcher {
    /// Library watch; dropping it stops the notifier.
    watcher: Option<RecommendedWatcher>,
    /// Task that dispatches notifications.
    task: JoinHandle<()>,
}

impl ThemeWatcher for NativeWatcher {
    fn close(&mut self) -> io::Result<()> {
        self.watcher = None;
        self.task.abort();
        Ok(())
    }
}

impl Drop for NativeWatcher {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// A native timer task that is aborted when cancelled or dropped.
struct NativeTimer(JoinHandle<()>);

impl ThemeReloadTimer for NativeTimer {
    fn cancel(&mut self) {
        self.0.abort();
    }
}

impl Drop for NativeTimer {
    fn drop(&mut self) {
        self.0.abort();
    }
}
