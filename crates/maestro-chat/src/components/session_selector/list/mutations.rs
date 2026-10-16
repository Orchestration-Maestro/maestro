//! Confirmation-first mutation dispatch and replaceable callable slots.
use super::{Rc, RefCell, SessionList, canonicalize_path};
use maestro_tui::{
    get_keybindings,
    tui::{InputHandler, LocalFuture},
};
/// Borrowed confirmation target notification.
type Confirmation = Rc<dyn Fn(Option<&str>)>;
/// Owned asynchronous deletion factory.
type Delete = Rc<dyn Fn(&str) -> LocalFuture>;
/// Synchronous path or error notification.
type Notify = Rc<dyn Fn(&str)>;
/// Independent optional mutation slots.
#[derive(Default)]
pub(super) struct Mutations {
    /// Confirmation observer.
    confirmation: RefCell<Option<Confirmation>>,
    /// Deletion factory.
    delete: RefCell<Option<Delete>>,
    /// Rename notification.
    rename: RefCell<Option<Notify>>,
    /// Error notification.
    error: RefCell<Option<Notify>>,
}
impl SessionList {
    /// Returns the current confirmation observer.
    #[must_use]
    pub fn on_delete_confirmation_change(&self) -> Option<Confirmation> {
        self.mutations.confirmation.borrow().clone()
    }
    /// Replaces confirmation delivery, releasing the old capture outside the borrow.
    pub fn set_on_delete_confirmation_change(&self, callback: Option<Confirmation>) {
        let old = self.mutations.confirmation.replace(callback);
        drop(old);
    }
    /// Returns the deletion factory.
    #[must_use]
    pub fn on_delete_session(&self) -> Option<Delete> {
        self.mutations.delete.borrow().clone()
    }
    /// Replaces the asynchronous deletion factory.
    pub fn set_on_delete_session(&self, callback: Option<Delete>) {
        let old = self.mutations.delete.replace(callback);
        drop(old);
    }
    /// Returns the rename notification.
    #[must_use]
    pub fn on_rename_session(&self) -> Option<Notify> {
        self.mutations.rename.borrow().clone()
    }
    /// Replaces rename delivery.
    pub fn set_on_rename_session(&self, callback: Option<Notify>) {
        let old = self.mutations.rename.replace(callback);
        drop(old);
    }
    /// Returns the error notification.
    #[must_use]
    pub fn on_error(&self) -> Option<Notify> {
        self.mutations.error.borrow().clone()
    }
    /// Replaces error delivery.
    pub fn set_on_error(&self, callback: Option<Notify>) {
        let old = self.mutations.error.replace(callback);
        drop(old);
    }
    /// Uses captured canonical active identity without changing authored operands.
    pub(super) fn is_current_path(&self, path: &str) -> bool {
        self.current
            .as_ref()
            .is_some_and(|current| canonicalize_path(path, self.options.paths.as_ref()) == *current)
    }
    /// Updates confirmation before synchronous notification.
    fn set_confirmation(&self, path: Option<&str>) {
        let old = self.confirming.replace(path.map(str::to_owned));
        drop(old);
        if let Some(callback) = self.on_delete_confirmation_change() {
            callback(path);
        }
    }
    /// Refuses active sessions before retaining the original selected spelling.
    fn start_confirmation(&self) {
        let Some(path) = self.get_selected_session_path() else {
            return;
        };
        if self.is_current_path(&path) {
            if let Some(callback) = self.on_error() {
                callback("Cannot delete the currently active session");
            }
        } else {
            self.set_confirmation(Some(&path));
        }
    }
    /// Consumes every key while confirming, including unrelated controls.
    pub(super) fn confirmation_input(&self, data: &str) -> bool {
        let Some(path) = self.confirming.borrow().clone() else {
            return false;
        };
        let bindings = get_keybindings();
        if bindings.matches(data, "tui.select.confirm") {
            self.set_confirmation(None);
            if let Some(callback) = self.on_delete_session() {
                self.options.runtime.spawn_local(callback(&path));
            }
        } else if bindings.matches(data, "tui.select.cancel") {
            self.set_confirmation(None);
        }
        true
    }
    /// Dispatches mutation keys after ordinary controls and before navigation.
    pub(super) fn mutation_input(&self, data: &str) -> bool {
        let bindings = get_keybindings();
        if bindings.matches(data, "app.session.delete") {
            self.start_confirmation();
        } else if bindings.matches(data, "app.session.rename") {
            if let (Some(path), Some(callback)) =
                (self.get_selected_session_path(), self.on_rename_session())
            {
                callback(&path);
            }
        } else if bindings.matches(data, "app.session.deleteNoninvasive") {
            if self.input.get_value().is_empty() {
                self.start_confirmation();
            } else {
                self.input.handle_input(data);
                self.filter_sessions();
            }
        } else {
            return false;
        }
        true
    }
}
