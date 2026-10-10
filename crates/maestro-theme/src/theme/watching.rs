//! Custom-theme file watching with a debounced reload that keeps the last good theme.
use super::fs_watch::{
    ThemeReloadTimer, ThemeWatchOperations, ThemeWatcher, close_watcher, watch_with_error_handler,
};
use super::{ThemeState, load_theme_from_path};
use maestro_path::join;
use std::rc::{Rc, Weak};
use std::time::Duration;

/// Wait after the last notification before rereading the file.
const RELOAD_DEBOUNCE: Duration = Duration::from_millis(100);

/// The active directory watch, its pending reload and the effects that created them.
#[derive(Default)]
pub(super) struct Watch {
    /// Directory watch of the selected custom theme.
    watcher: Option<Box<dyn ThemeWatcher>>,
    /// Pending debounced reload.
    timer: Option<Box<dyn ThemeReloadTimer>>,
    /// Effects kept alive for as long as the watch is.
    operations: Option<Rc<dyn ThemeWatchOperations>>,
}

impl ThemeState {
    /// Cancel the pending reload and close the watch; the published theme and the
    /// change callback stay.
    pub fn stop_theme_watcher(&self) {
        let Watch {
            watcher,
            timer,
            operations,
        } = std::mem::take(&mut *self.lifecycle.watch.borrow_mut());
        if let Some(mut timer) = timer {
            timer.cancel();
        }
        close_watcher(watcher);
        drop(operations);
    }

    /// Replace the watch with one on the selected custom theme's file, when it exists.
    pub(super) fn start_theme_watcher(
        self: &Rc<Self>,
        operations: Option<Rc<dyn ThemeWatchOperations>>,
    ) {
        let Some(operations) = operations else {
            return;
        };
        self.stop_theme_watcher();
        let Some(name) = self.lifecycle.name.borrow().clone() else {
            return;
        };
        if name.is_empty() || name == "dark" || name == "light" {
            return;
        }
        let dir = &self.directories.custom_themes_dir;
        let file = format!("{name}.json");
        if !self.operations.exists(&join(&[dir, &file])) {
            return;
        }
        let (state, effects) = (Rc::downgrade(self), Rc::downgrade(&operations));
        let on_error = Weak::clone(&state);
        let watcher = watch_with_error_handler(
            dir,
            Rc::new(move |changed| {
                if let (Some(state), Some(effects)) = (state.upgrade(), effects.upgrade()) {
                    state.notified(&effects, &name, &file, changed.as_deref());
                }
            }),
            Rc::new(move || {
                if let Some(state) = on_error.upgrade() {
                    let watcher = state.lifecycle.watch.borrow_mut().watcher.take();
                    close_watcher(watcher);
                }
            }),
            operations.as_ref(),
        );
        let mut slot = self.lifecycle.watch.borrow_mut();
        slot.watcher = watcher;
        slot.operations = Some(operations);
    }

    /// Schedule a reload of `file` for a notification about it or an unknown entry,
    /// while `name` is still the selected theme.
    fn notified(
        self: &Rc<Self>,
        operations: &Rc<dyn ThemeWatchOperations>,
        name: &str,
        file: &str,
        changed: Option<&str>,
    ) {
        if self.lifecycle.name.borrow().as_deref() != Some(name) {
            return;
        }
        if changed.is_some_and(|changed| !changed.is_empty() && changed != file) {
            return;
        }
        let previous = self.lifecycle.watch.borrow_mut().timer.take();
        if let Some(mut previous) = previous {
            previous.cancel();
        }
        let weak = Rc::downgrade(self);
        let name = name.to_owned();
        let path = join(&[&self.directories.custom_themes_dir, file]);
        let timer = operations.schedule(
            RELOAD_DEBOUNCE,
            Box::new(move || {
                if let Some(state) = weak.upgrade() {
                    state.reload(&name, &path);
                }
            }),
        );
        self.lifecycle.watch.borrow_mut().timer = Some(timer);
    }
}

impl ThemeState {
    /// Reread the selected file and publish it; any failure keeps the last good theme.
    fn reload(&self, name: &str, path: &str) {
        let fired = self.lifecycle.watch.borrow_mut().timer.take();
        drop(fired);
        if self.lifecycle.name.borrow().as_deref() != Some(name) || !self.operations.exists(path) {
            return;
        }
        if let Ok(theme) = load_theme_from_path(path, None, self.operations.as_ref()) {
            let theme = Rc::new(theme);
            self.registered
                .borrow_mut()
                .insert(name.to_owned(), Rc::clone(&theme));
            self.publish(theme);
            let _ = self.notify();
        }
    }
}

impl Drop for ThemeState {
    fn drop(&mut self) {
        self.stop_theme_watcher();
    }
}
