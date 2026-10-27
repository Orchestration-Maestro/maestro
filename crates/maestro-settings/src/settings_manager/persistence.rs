//! Captured revisions, the single write queue and fresh-storage merging.

use std::future::Future;
#[cfg(not(target_arch = "wasm32"))]
use std::io;

use serde_json::{Map, Value};
use tokio::sync::oneshot;

use super::BaseSettingsLayer;
use super::change::Edit;
use super::conversion::parse_document;
use super::preferences::SettingsStorageHandle;
use super::preferences::{SettingsError, SettingsScope, SettingsStorage, SettingsStorageError};
use maestro_json::pretty_object;

/// Shared state handle: atomically counted and locked on native targets.
#[cfg(not(target_arch = "wasm32"))]
type Cell<T> = std::sync::Arc<std::sync::Mutex<T>>;
/// Shared state handle: reference counted and borrow checked in the browser.
#[cfg(target_arch = "wasm32")]
type Cell<T> = std::rc::Rc<std::cell::RefCell<T>>;

/// The monotonic number that distinguishes two edits of the same preference.
type Revision = u64;

/// An edit together with the revision that was current when it was captured.
type Captured = (Edit, Revision);

/// State shared between the manager and the queue worker.
#[derive(Default)]
struct Shared {
    /// The last revision handed out.
    revision: Revision,
    /// Unsaved edits per scope, in first-edit order.
    dirty: [Vec<Captured>; 2],
    /// Failures not yet drained.
    errors: Vec<SettingsError>,
}

impl Shared {
    /// Records an edit under a new revision.
    fn mark(&mut self, scope: SettingsScope, edit: Edit) {
        self.revision += 1;
        let revision = self.revision;
        let dirty = &mut self.dirty[scope as usize];
        match dirty.iter_mut().find(|(known, _)| *known == edit) {
            Some(entry) => entry.1 = revision,
            None => dirty.push((edit, revision)),
        }
    }

    /// Forgets exactly the captured edits, keeping any that changed since.
    fn acknowledge(&mut self, scope: SettingsScope, captured: &[Captured]) {
        self.dirty[scope as usize].retain(|entry| !captured.contains(entry));
    }
}

/// Everything the queue worker needs.
#[derive(Clone)]
struct Context {
    /// Where saves are written.
    storage: SettingsStorageHandle,
    /// Dirty edits and errors.
    shared: Cell<Shared>,
}

impl Context {
    /// Runs `update` on the shared state; callers never run foreign code inside.
    fn with<R>(&self, update: impl FnOnce(&mut Shared) -> R) -> R {
        #[cfg(not(target_arch = "wasm32"))]
        let mut shared = self
            .shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        #[cfg(target_arch = "wasm32")]
        let mut shared = self.shared.borrow_mut();
        update(&mut shared)
    }
}

/// One unit of queued work.
enum Job {
    /// Merge captured edits into freshly read storage and write them.
    Save {
        /// The scope to write.
        scope: SettingsScope,
        /// The scope's accepted settings when the save was enqueued.
        snapshot: Map<String, Value>,
        /// The edits and revisions captured when the save was enqueued.
        edits: Vec<Captured>,
    },
    /// Persists a published layer without touching preference edits.
    BaseLayer {
        /// The publication's scope.
        scope: SettingsScope,
        /// The accepted record, or absence to clear it.
        layer: Option<BaseSettingsLayer>,
    },
    /// Completes once every earlier job has run.
    Barrier(oneshot::Sender<()>),
}

/// The ordered write queue and the edits waiting to be saved.
pub(super) struct Queue {
    /// State shared with the worker.
    context: Context,
    /// Hands jobs to the worker in order.
    dispatcher: Dispatcher,
}

impl Queue {
    /// Creates an empty queue over a storage handle.
    pub(super) fn new(storage: SettingsStorageHandle) -> Self {
        Self {
            context: Context {
                storage,
                shared: Cell::default(),
            },
            dispatcher: Dispatcher::default(),
        }
    }

    /// Marks an edit as unsaved.
    pub(super) fn mark(&self, scope: SettingsScope, edit: Edit) {
        self.context.with(|shared| shared.mark(scope, edit));
    }

    /// Forgets every unsaved edit.
    pub(super) fn clear_dirty(&self) {
        self.context
            .with(|shared| shared.dirty = Default::default());
    }

    /// Records a failure for later draining.
    pub(super) fn record_error(&self, scope: SettingsScope, error: SettingsStorageError) {
        self.context
            .with(|shared| shared.errors.push(SettingsError { scope, error }));
    }

    /// Takes every recorded failure.
    pub(super) fn drain_errors(&self) -> Vec<SettingsError> {
        self.context
            .with(|shared| std::mem::take(&mut shared.errors))
    }

    /// Queues a save of the edits that are unsaved right now.
    pub(super) fn save(&mut self, scope: SettingsScope, snapshot: Map<String, Value>) {
        let edits = self
            .context
            .with(|shared| shared.dirty[scope as usize].clone());
        let job = Job::Save {
            scope,
            snapshot,
            edits,
        };
        self.send(scope, job);
    }

    /// Queues a replacement or removal of a base record on the same worker.
    pub(super) fn save_base_layer(
        &mut self,
        scope: SettingsScope,
        layer: Option<BaseSettingsLayer>,
    ) {
        self.send(scope, Job::BaseLayer { scope, layer });
    }

    /// Dispatches one job, recording any worker-start failure.
    fn send(&mut self, scope: SettingsScope, job: Job) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Err(error) = self.dispatcher.send(&self.context, job) {
            self.record_error(scope, Box::new(error));
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = scope;
            self.dispatcher.send(&self.context, job);
        }
    }

    /// Returns a future that completes once the work queued so far has run.
    pub(super) fn barrier(&self) -> impl Future<Output = ()> + use<> {
        let (done, wait) = oneshot::channel();
        // Without a worker nothing is pending; a dead worker drops the job,
        // which completes the wait immediately.
        self.dispatcher
            .send_started(&self.context, Job::Barrier(done));
        async move {
            let _ = wait.await;
        }
    }
}

/// Runs one job against storage and updates the shared state.
fn execute(job: Job, context: &Context) {
    match job {
        Job::Barrier(done) => {
            let _ = done.send(());
        }
        Job::BaseLayer { scope, layer } => {
            if let Err(error) = context.storage.write_base_layer(scope, layer.as_ref()) {
                context.with(|shared| shared.errors.push(SettingsError { scope, error }));
            }
        }
        Job::Save {
            scope,
            snapshot,
            edits,
        } => match persist(&*context.storage, scope, &snapshot, &edits) {
            Ok(()) => context.with(|shared| shared.acknowledge(scope, &edits)),
            Err(error) => context.with(|shared| {
                shared.errors.push(SettingsError { scope, error });
            }),
        },
    }
}

/// Merges the captured edits into the freshly read scope and writes it back.
fn persist(
    storage: &dyn SettingsStorage,
    scope: SettingsScope,
    snapshot: &Map<String, Value>,
    edits: &[Captured],
) -> Result<(), SettingsStorageError> {
    storage.with_write_lock(scope, &mut |current| {
        let mut fresh = parse_document(current.unwrap_or_default())?;
        for (edit, _) in edits {
            edit.apply(&mut fresh, edit.value_in(snapshot).cloned());
        }
        Ok(Some(pretty_object(&fresh)?))
    })
}

/// Hands jobs to one background worker thread.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
struct Dispatcher {
    /// Present once the worker has been started.
    sender: Option<std::sync::mpsc::Sender<Job>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Dispatcher {
    /// Queues a job only if the worker already exists.
    fn send_started(&self, _context: &Context, job: Job) {
        if let Some(sender) = &self.sender {
            let _ = sender.send(job);
        }
    }

    /// Queues a job, starting the worker on first use.
    fn send(&mut self, context: &Context, job: Job) -> io::Result<()> {
        if self.sender.is_none() {
            self.sender = Some(Self::start(context)?);
        }
        let failed = self
            .sender
            .as_ref()
            .is_some_and(|sender| sender.send(job).is_err());
        if failed {
            self.sender = None;
            return Err(io::Error::other("the settings queue worker stopped"));
        }
        Ok(())
    }

    /// Starts the worker thread and returns the sender that feeds it.
    fn start(context: &Context) -> io::Result<std::sync::mpsc::Sender<Job>> {
        let (sender, receiver) = std::sync::mpsc::channel();
        let context = context.clone();
        std::thread::Builder::new()
            .name("maestro-settings-queue".to_owned())
            .spawn(move || receiver.into_iter().for_each(|job| execute(job, &context)))?;
        Ok(sender)
    }
}

/// Runs jobs on the browser's task queue.
#[cfg(target_arch = "wasm32")]
#[derive(Default)]
struct Dispatcher {
    /// Jobs waiting for the drain task.
    pending: std::rc::Rc<std::cell::RefCell<std::collections::VecDeque<Job>>>,
    /// Whether a drain task is scheduled.
    running: std::rc::Rc<std::cell::Cell<bool>>,
}

#[cfg(target_arch = "wasm32")]
impl Dispatcher {
    /// Queues a job; the browser queue always exists.
    fn send_started(&self, context: &Context, job: Job) {
        self.send(context, job);
    }

    /// Queues a job and schedules the drain task if none is running.
    fn send(&self, context: &Context, job: Job) {
        self.pending.borrow_mut().push_back(job);
        if !self.running.replace(true) {
            let (pending, running) = (self.pending.clone(), self.running.clone());
            let context = context.clone();
            wasm_bindgen_futures::spawn_local(async move { drain(&pending, &running, &context) });
        }
    }
}

/// Runs every pending job, then marks the drain task as finished.
#[cfg(target_arch = "wasm32")]
fn drain(
    pending: &std::cell::RefCell<std::collections::VecDeque<Job>>,
    running: &std::cell::Cell<bool>,
    context: &Context,
) {
    loop {
        let next = pending.borrow_mut().pop_front();
        let Some(job) = next else { break };
        execute(job, context);
    }
    running.set(false);
}
