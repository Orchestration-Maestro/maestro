//! Ordered session control dispatch and replaceable observer slots.
use super::{Rc, RefCell, SessionList};
use maestro_tui::{KeybindingsManager, get_keybindings};
/// Optional no-argument control notification.
type Toggle = Option<Rc<dyn Fn()>>;
/// Optional path-display observer.
type PathToggle = Option<Rc<dyn Fn(bool)>>;
/// Retained manager and independent public callback slots.
pub(super) struct Controls {
    /// Named filtering's live manager identity.
    pub(super) keybindings: RefCell<KeybindingsManager>,
    /// Explicit exit, without an associated input binding.
    exit: RefCell<Rc<dyn Fn()>>,
    /// Scope selection notification.
    scope: RefCell<Toggle>,
    /// Ordering notification.
    sort: RefCell<Toggle>,
    /// Name membership notification.
    name: RefCell<Toggle>,
    /// Path display notification.
    path: RefCell<PathToggle>,
}
impl Controls {
    /// Starts with no optional observers and a callable no-op exit.
    pub(super) fn new() -> Self {
        Self {
            keybindings: RefCell::new(get_keybindings()),
            exit: RefCell::new(Rc::new(|| {})),
            scope: RefCell::default(),
            sort: RefCell::default(),
            name: RefCell::default(),
            path: RefCell::default(),
        }
    }
}
impl SessionList {
    /// Returns the current exit notification handle.
    #[must_use]
    pub fn on_exit(&self) -> Rc<dyn Fn()> {
        self.controls.exit.borrow().clone()
    }
    /// Replaces exit delivery, releasing the old capture outside the borrow.
    pub fn set_on_exit(&self, callback: Rc<dyn Fn()>) {
        let old = self.controls.exit.replace(callback);
        drop(old);
    }
    /// Returns the current scope observer.
    #[must_use]
    pub fn on_toggle_scope(&self) -> Toggle {
        self.controls.scope.borrow().clone()
    }
    /// Replaces or clears scope delivery.
    pub fn set_on_toggle_scope(&self, callback: Toggle) {
        let old = self.controls.scope.replace(callback);
        drop(old);
    }
    /// Returns the current ordering observer.
    #[must_use]
    pub fn on_toggle_sort(&self) -> Toggle {
        self.controls.sort.borrow().clone()
    }
    /// Replaces or clears ordering delivery.
    pub fn set_on_toggle_sort(&self, callback: Toggle) {
        let old = self.controls.sort.replace(callback);
        drop(old);
    }
    /// Returns the current membership observer.
    #[must_use]
    pub fn on_toggle_name_filter(&self) -> Toggle {
        self.controls.name.borrow().clone()
    }
    /// Replaces or clears membership delivery.
    pub fn set_on_toggle_name_filter(&self, callback: Toggle) {
        let old = self.controls.name.replace(callback);
        drop(old);
    }
    /// Returns the current path-display observer.
    #[must_use]
    pub fn on_toggle_path(&self) -> PathToggle {
        self.controls.path.borrow().clone()
    }
    /// Replaces or clears path-display delivery.
    pub fn set_on_toggle_path(&self, callback: PathToggle) {
        let old = self.controls.path.replace(callback);
        drop(old);
    }
    /// Consumes recognized controls in precedence order before ordinary input.
    pub(super) fn control_input(&self, data: &str) -> bool {
        let global = get_keybindings();
        let supplied = self.controls.keybindings.borrow().clone();
        let callback = if global.matches(data, "tui.input.tab") {
            self.on_toggle_scope()
        } else if global.matches(data, "app.session.toggleSort") {
            self.on_toggle_sort()
        } else if supplied.matches(data, "app.session.toggleNamedFilter") {
            self.on_toggle_name_filter()
        } else if global.matches(data, "app.session.togglePath") {
            let show = !self.show_path.get();
            self.show_path.set(show);
            if let Some(callback) = self.on_toggle_path() {
                callback(show);
            }
            return true;
        } else {
            return false;
        };
        if let Some(callback) = callback {
            callback();
        }
        true
    }
}
