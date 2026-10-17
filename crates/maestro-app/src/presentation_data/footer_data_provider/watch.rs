//! Footer-owned watch registrations and coalesced branch refresh.

use super::{
    git,
    operations::FooterOperations,
    provider::{Callbacks, Lookup, State},
};
use maestro_watch::fs_watch::{
    FS_WATCH_RETRY_DELAY_MS, FsWatcher, WatchTimer, close_watcher, watch_with_error_handler,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

/// Registered effects and refresh state shared by provider aliases.
pub(super) struct Watching {
    /// Replaceable effects.
    operations: Rc<dyn FooterOperations>,
    /// Live repository and cached branch.
    state: Rc<RefCell<State>>,
    /// Live subscription collection.
    pub callbacks: Callbacks,
    /// Independently owned watch and poll registrations.
    sources: RefCell<Vec<Box<dyn FsWatcher>>>,
    /// The first-event refresh deadline.
    refresh: RefCell<Option<Box<dyn WatchTimer>>>,
    /// The next failed-registration retry.
    retry: RefCell<Option<Box<dyn WatchTimer>>>,
    /// Working-directory identity captured by each query.
    generation: Cell<u64>,
    /// Whether a branch query awaits completion.
    in_flight: Cell<bool>,
    /// Whether an event arrived during that query.
    pending: Cell<bool>,
    /// Whether watch work has ended.
    disposed: Cell<bool>,
}
impl Watching {
    /// Create an owner without installing effects.
    pub fn new(
        operations: Rc<dyn FooterOperations>,
        state: Rc<RefCell<State>>,
        callbacks: Callbacks,
    ) -> Rc<Self> {
        Rc::new(Self {
            operations,
            state,
            callbacks,
            sources: RefCell::default(),
            refresh: RefCell::default(),
            retry: RefCell::default(),
            generation: Cell::new(0),
            in_flight: Cell::new(false),
            pending: Cell::new(false),
            disposed: Cell::new(false),
        })
    }
    /// Schedule an action without retaining the owner.
    fn timer(self: &Rc<Self>, millis: u64, run: fn(&Rc<Self>)) -> Box<dyn WatchTimer> {
        let weak = Rc::downgrade(self);
        self.operations.schedule(
            Duration::from_millis(millis),
            Box::new(move || {
                if let Some(owner) = weak.upgrade() {
                    run(&owner);
                }
            }),
        )
    }
    /// Close registrations and cancel retry, retaining any branch-refresh deadline.
    pub fn clear(&self) {
        let sources = std::mem::take(&mut *self.sources.borrow_mut());
        for source in sources {
            close_watcher(Some(source));
        }
        cancel(&self.retry);
    }
    /// Invalidate query publication and release old-directory scheduled work.
    pub fn changed_cwd(&self) {
        self.generation.set(self.generation.get() + 1);
        cancel(&self.refresh);
        self.clear();
    }
    /// Stop watch work and clear live subscriptions.
    pub fn dispose(&self) {
        self.disposed.set(true);
        cancel(&self.refresh);
        self.clear();
        self.callbacks.clear();
    }
    /// Release failed registrations and schedule another attempt unless disposed.
    fn error(self: &Rc<Self>) {
        self.clear();
        if !self.disposed.get() && self.retry.borrow().is_none() {
            *self.retry.borrow_mut() = Some(self.timer(FS_WATCH_RETRY_DELAY_MS, Self::setup));
        }
    }
    /// Coalesce events at the first deadline or queue a follow-up to an in-flight query.
    pub fn schedule(self: &Rc<Self>) {
        if self.disposed.get() || self.refresh.borrow().is_some() {
            return;
        }
        if self.in_flight.get() {
            self.pending.set(true);
            return;
        }
        *self.refresh.borrow_mut() = Some(self.timer(500, Self::refresh));
    }
    /// Select HEAD or launch a symbolic-reference query for the captured generation.
    fn refresh(self: &Rc<Self>) {
        self.refresh.borrow_mut().take();
        if self.disposed.get() {
            return;
        }
        self.in_flight.set(true);
        let generation = self.generation.get();
        let repo = {
            let state = self.state.borrow();
            let selection = state
                .git_paths
                .as_ref()
                .map_or(git::BranchRead::Ready(None), |paths| {
                    git::read_branch(self.operations.as_ref(), paths)
                });
            match selection {
                git::BranchRead::Ready(branch) => {
                    drop(state);
                    return self.finish(generation, branch);
                }
                git::BranchRead::Query(repo) => repo.to_owned(),
            }
        };
        let weak = Rc::downgrade(self);
        self.operations.symbolic_ref_async(
            &repo,
            Box::new(move |result| {
                if let Some(owner) = weak.upgrade() {
                    owner.finish(generation, Some(git::git_answer(result.ok().flatten())));
                }
            }),
        );
    }

    /// Publish a current completion before notifying the live callback collection.
    fn finish(self: &Rc<Self>, generation: u64, branch: Option<String>) {
        let changed = !self.disposed.get() && self.generation.get() == generation && {
            let mut state = self.state.borrow_mut();
            let changed = matches!(&state.branch, Lookup::Done(previous) if *previous != branch);
            state.branch = Lookup::Done(branch);
            changed
        };
        if changed {
            for (_, callback) in &self.callbacks {
                callback();
            }
        }
        self.in_flight.set(false);
        if self.pending.replace(false) {
            self.schedule();
        }
    }
    /// Register a source with HEAD filename filtering when requested.
    fn add(self: &Rc<Self>, path: &str, head: bool) -> bool {
        let weak = Rc::downgrade(self);
        let listener = Rc::new(move |name: Option<String>| {
            let relevant = !head || name.as_deref().is_none_or(|n| n.is_empty() || n == "HEAD");
            if let Some(owner) = weak.upgrade().filter(|_| relevant) {
                owner.schedule();
            }
        });
        let weak = Rc::downgrade(self);
        let error = Rc::new(move || {
            if let Some(owner) = weak.upgrade() {
                owner.error();
            }
        });
        watch_with_error_handler(path, listener, error, self.operations.as_ref())
            .map(|watcher| self.sources.borrow_mut().push(watcher))
            .is_some()
    }
    /// Install available repository watches in dependency order.
    pub fn setup(self: &Rc<Self>) {
        self.clear();
        if self.disposed.get() {
            return;
        }
        let Some((head, reftable)) = self.state.borrow().git_paths.as_ref().map(|paths| {
            (
                maestro_path::dirname(&paths.head_path),
                maestro_path::join(&[&paths.common_git_dir, "reftable"]),
            )
        }) else {
            return;
        };
        if !self.add(&head, true)
            || !self.operations.exists(&reftable)
            || !self.add(&reftable, false)
        {
            return;
        }
        let tables = maestro_path::join(&[&reftable, "tables.list"]);
        if !self.operations.exists(&tables) || !self.add(&tables, false) {
            return;
        }
        let weak = Rc::downgrade(self);
        let listener = Rc::new(move |now, previous| {
            if now != previous
                && let Some(owner) = weak.upgrade()
            {
                owner.schedule();
            }
        });
        let poll = self
            .operations
            .watch_file(&tables, Duration::from_millis(250), listener);
        self.sources.borrow_mut().push(poll);
    }
}
/// Remove the timer slot before cancelling its effect.
fn cancel(slot: &RefCell<Option<Box<dyn WatchTimer>>>) {
    let timer = slot.borrow_mut().take();
    if let Some(mut timer) = timer {
        timer.cancel();
    }
}
impl Drop for Watching {
    fn drop(&mut self) {
        self.dispose();
    }
}
