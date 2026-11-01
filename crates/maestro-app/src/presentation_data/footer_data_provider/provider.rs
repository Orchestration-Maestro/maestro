//! The footer metadata owner.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use maestro_extensions::footer_data_provider::live::Live;

use super::git::{GitPaths, find_git_paths, resolve_branch};
use super::operations::FooterOperations;
use super::{ExtensionStatuses, ReadonlyFooterDataProvider};

/// Branch-change callbacks keyed by the identity of their `Rc`.
pub(super) type Callbacks = Live<usize, Rc<dyn Fn()>>;

/// The identity a callback is registered and unsubscribed by.
fn identity(callback: &Rc<dyn Fn()>) -> usize {
    Rc::as_ptr(callback).addr()
}

/// Whether the branch has been looked up since the working directory changed.
pub(super) enum Lookup {
    /// No lookup yet.
    Pending,
    /// The branch found, which is absent outside a repository.
    Done(Option<String>),
}

/// Mutable provider state; never borrowed while a callback runs.
pub(super) struct State {
    /// The working directory as authored.
    cwd: String,
    /// The repository found from `cwd`.
    pub(super) git_paths: Option<GitPaths>,
    /// The branch lookup of `cwd`, once made.
    pub(super) branch: Lookup,
}

impl State {
    /// The state of `cwd`, with its repository discovered and no branch looked up.
    fn new(cwd: String) -> Self {
        Self {
            git_paths: find_git_paths(&cwd),
            cwd,
            branch: Lookup::Pending,
        }
    }
}

/// Owner of the footer metadata. Clones share one state.
#[derive(Clone)]
pub struct FooterDataProvider {
    /// Watch lifecycle shared by provider and unsubscribe aliases.
    watch: Rc<super::watch::Watching>,
    /// The supplied watch registration, timer scheduling, polling and Git process effects.
    operations: Rc<dyn FooterOperations>,
    /// The working directory, repository and cached branch.
    state: Rc<RefCell<State>>,
    /// The extension status texts.
    statuses: ExtensionStatuses,
    /// The branch-change subscriptions.
    callbacks: Callbacks,
    /// The supplied provider count.
    available_provider_count: Rc<Cell<f64>>,
}

impl FooterDataProvider {
    /// Discover metadata and install available watches for `cwd`. Branch lookup
    /// remains lazy until a getter or refresh.
    #[must_use]
    pub fn new(cwd: String, operations: Rc<dyn FooterOperations>) -> Self {
        let state = Rc::new(RefCell::new(State::new(cwd)));
        let callbacks = Live::new();
        let watch = super::watch::Watching::new(
            Rc::clone(&operations),
            Rc::clone(&state),
            callbacks.clone(),
        );
        watch.setup();
        Self {
            operations,
            state,
            callbacks,
            watch,
            statuses: Live::new(),
            available_provider_count: Rc::new(Cell::new(0.0)),
        }
    }

    /// Set the text of extension status `key`, or remove it with `None`.
    pub fn set_extension_status(&self, key: &str, text: Option<&str>) {
        match text {
            Some(text) => self.statuses.insert(key.to_owned(), text.to_owned()),
            None => drop(self.statuses.remove(key)),
        }
    }

    /// Remove every extension status.
    pub fn clear_extension_statuses(&self) {
        self.statuses.clear();
    }

    /// Store `count` as supplied.
    pub fn set_available_provider_count(&self, count: f64) {
        self.available_provider_count.set(count);
    }

    /// Move to `cwd`. The same spelling changes nothing; otherwise the cached
    /// branch is invalidated before the branch-change callbacks run in
    /// registration order, including callbacks added while they run.
    pub fn set_cwd(&self, cwd: String) {
        if self.state.borrow().cwd == cwd {
            return;
        }
        self.watch.changed_cwd();
        *self.state.borrow_mut() = State::new(cwd);
        self.watch.setup();
        for (_, callback) in &self.callbacks {
            callback();
        }
    }

    /// End watch work and clear subscriptions. Statuses, the count and the
    /// cached branch stay readable.
    pub fn dispose(&self) {
        self.watch.dispose();
    }
}

impl ReadonlyFooterDataProvider for FooterDataProvider {
    fn get_git_branch(&self) -> Option<String> {
        let mut state = self.state.borrow_mut();
        if let Lookup::Done(branch) = &state.branch {
            return branch.clone();
        }
        let branch = state
            .git_paths
            .as_ref()
            .and_then(|paths| resolve_branch(self.operations.as_ref(), paths));
        state.branch = Lookup::Done(branch.clone());
        branch
    }

    fn get_extension_statuses(&self) -> ExtensionStatuses {
        self.statuses.clone()
    }

    fn get_available_provider_count(&self) -> f64 {
        self.available_provider_count.get()
    }

    fn on_branch_change(&self, callback: Rc<dyn Fn()>) -> Box<dyn Fn()> {
        self.callbacks
            .insert(identity(&callback), Rc::clone(&callback));
        let watch = Rc::clone(&self.watch);
        Box::new(move || {
            drop(watch.callbacks.remove(&identity(&callback)));
        })
    }
}
