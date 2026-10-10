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
    pub(super) fn start_theme_watcher(self: &Rc<Self>, operations: &Rc<dyn ThemeWatchOperations>) {
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
        self.lifecycle.watch.borrow_mut().operations = Some(Rc::clone(operations));
        let target = Target {
            state: Rc::downgrade(self),
            operations: Rc::downgrade(operations),
            name,
            file,
        };
        let on_error = Weak::clone(&target.state);
        let watcher = watch_with_error_handler(
            dir,
            Rc::new(move |changed| target.notified(changed.as_deref())),
            Rc::new(move || {
                if let Some(state) = on_error.upgrade() {
                    let watcher = state.lifecycle.watch.borrow_mut().watcher.take();
                    close_watcher(watcher);
                }
            }),
            operations.as_ref(),
        );
        self.lifecycle.watch.borrow_mut().watcher = watcher;
    }
}

/// The selected custom theme file and the weak handles a watch callback captures.
struct Target {
    /// Owner of the watch.
    state: Weak<ThemeState>,
    /// Effects that schedule the debounced reload.
    operations: Weak<dyn ThemeWatchOperations>,
    /// Selected theme name when the watch started.
    name: String,
    /// File name inside the custom directory.
    file: String,
}

impl Target {
    /// Schedule a reload for a notification about this file or an unknown entry.
    fn notified(&self, changed: Option<&str>) {
        let (Some(state), Some(operations)) = (self.state.upgrade(), self.operations.upgrade())
        else {
            return;
        };
        if state.lifecycle.name.borrow().as_deref() != Some(self.name.as_str()) {
            return;
        }
        if changed.is_some_and(|changed| !changed.is_empty() && changed != self.file) {
            return;
        }
        let previous = state.lifecycle.watch.borrow_mut().timer.take();
        if let Some(mut previous) = previous {
            previous.cancel();
        }
        let weak = Weak::clone(&self.state);
        let name = self.name.clone();
        let path = join(&[&state.directories.custom_themes_dir, &self.file]);
        let timer = operations.schedule(
            RELOAD_DEBOUNCE,
            Box::new(move || {
                if let Some(state) = weak.upgrade() {
                    state.reload(&name, &path);
                }
            }),
        );
        state.lifecycle.watch.borrow_mut().timer = Some(timer);
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
