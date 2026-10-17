//! Single-thread owners, buses and runners shared by local test programs.
#![cfg(test)]
use maestro_extensions::{
    EventBus, EventBusController, EventListener, Extension, ExtensionAPI, ExtensionRunner,
    SourceInfo, create_event_bus_for,
    types::{ExtensionCollection, ExtensionHandle, ExtensionRuntimeState},
};
use maestro_models::Local;
use maestro_theme::LiveTheme;
use serde_json::json;
use std::{
    cell::{Cell, RefCell},
    future::Future,
    rc::Rc,
    sync::{Arc, RwLock},
};
use tokio::task::JoinHandle;

/// Real in-memory session operands.
#[path = "session.rs"]
pub mod session;

/// The complete default stale-context guidance.
pub const STALE: &str = "This extension ctx is stale after session replacement or reload. Do not use a captured maestro or command ctx after ctx.newSession(), ctx.fork(), ctx.switchSession(), or ctx.reload(). For newSession, fork, and switchSession, move post-replacement work into withSession and use the ctx passed to withSession. For reload, do not use the old ctx after await ctx.reload().";

/// A local single-thread trace.
pub type Trace = Rc<RefCell<Vec<String>>>;

/// Runs a local test on one thread with a local task set.
pub fn local<F: Future<Output = ()>>(test: F) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, test);
}

/// Appends one observation.
pub fn mark(trace: &Trace, value: impl Into<String>) {
    trace.borrow_mut().push(value.into());
}

/// Supplies a temporary command-line source record.
pub fn source(path: &str) -> Arc<RwLock<SourceInfo>> {
    Arc::new(RwLock::new(
        serde_json::from_value(json!({
            "path": path, "source": "cli", "scope": "temporary", "origin": "top-level"
        }))
        .unwrap(),
    ))
}

/// A loaded owner with its facade, current collection and runtime state.
pub struct Loaded<C> {
    /// The strong registration owner.
    pub owner: ExtensionHandle<C, Local>,
    /// The weak registration facade.
    pub api: ExtensionAPI<C, Local>,
    /// Current load order containing the owner.
    pub owners: ExtensionCollection<C, Local>,
    /// Shared flag values and activity.
    pub runtime: Arc<ExtensionRuntimeState>,
}

/// Creates a loaded owner using the supplied bus identity.
pub fn loaded<C>(path: &str, events: EventBus<Local>) -> Loaded<C> {
    let owner = Rc::new(RwLock::new(Extension::new(
        path.into(),
        format!("{path}/index"),
        source(path),
    )));
    let runtime = Arc::new(ExtensionRuntimeState::new());
    Loaded {
        api: ExtensionAPI::new(&owner, Arc::clone(&runtime), events),
        owners: Rc::new(RwLock::new(vec![owner.clone()])),
        owner,
        runtime,
    }
}

/// Creates a local runner over the supplied collection, serving `/work`.
pub fn runner<C>(owners: &ExtensionCollection<C, Local>) -> ExtensionRunner<C, Local> {
    ExtensionRunner::<C, Local>::new(
        owners.clone(),
        Arc::new(ExtensionRuntimeState::new()),
        session::runner_session("/work").0,
        Rc::new(|_| {}),
        LiveTheme::default(),
    )
}

/// A local bus whose spawner, reporter and tails capture single-thread state.
pub struct LocalBus {
    /// The shared bus.
    pub controller: EventBusController<Local>,
    /// Admitted tail tasks.
    pub tasks: Rc<RefCell<Vec<JoinHandle<()>>>>,
    /// Reported diagnostics.
    pub errors: Trace,
    /// Number of spawner calls.
    pub spawned: Rc<Cell<usize>>,
}

impl LocalBus {
    /// Creates a bus that enqueues each tail once on the local task set.
    pub fn new() -> Self {
        let tasks = Rc::new(RefCell::new(Vec::new()));
        let errors = Trace::default();
        let spawned = Rc::new(Cell::new(0));
        let (queue, count, report) = (tasks.clone(), spawned.clone(), errors.clone());
        Self {
            controller: create_event_bus_for::<Local>(
                Rc::new(move |task| {
                    count.set(count.get() + 1);
                    queue.borrow_mut().push(tokio::task::spawn_local(task));
                }),
                Rc::new(move |line| mark(&report, line)),
            ),
            tasks,
            errors,
            spawned,
        }
    }

    /// Waits for every admitted tail, including tails admitted while waiting.
    pub async fn settle(&self) {
        loop {
            let tasks = std::mem::take(&mut *self.tasks.borrow_mut());
            if tasks.is_empty() {
                return;
            }
            for task in tasks {
                task.await.unwrap();
            }
        }
    }
}

/// A listener recording one label.
pub fn listener(trace: &Trace, label: &'static str) -> EventListener<Local> {
    let trace = trace.clone();
    Rc::new(move |_, _| {
        mark(&trace, label);
        Ok(None)
    })
}

/// Records its label when its last owner releases it.
pub struct Released(pub Rc<Cell<bool>>);

impl Drop for Released {
    fn drop(&mut self) {
        self.0.set(true);
    }
}
