//! Weak native presentation operations consumed by the existing application owner.
use super::{Entry, State};
use crate::tools::{artifacts::native::NativeArtifact, types::ToolUiError};
use maestro_app::{
    BoxFuture,
    browser::tools::artifacts::{
        ArtifactLogError, ArtifactLogReader, ArtifactPresentation, ArtifactsPanelState,
    },
};
use num_traits::ToPrimitive as _;
use std::{
    cell::RefCell,
    rc::{Rc, Weak},
    time::Duration,
};
use wasm_bindgen::JsCast as _;

/// Application operations never keep their native presentation alive.
pub(super) struct Presentation(pub(super) Weak<State>);
impl State {
    /// Upsert through the format owner, retaining one native element per literal name.
    fn upsert(
        &self,
        filename: &str,
        content: &str,
        panel: &ArtifactsPanelState,
    ) -> Result<(), ToolUiError> {
        let existing = self.entries.borrow().get(filename).cloned();
        let entry = existing.unwrap_or_else(|| {
            let entry = Rc::new(RefCell::new(Entry {
                controller: Rc::new(RefCell::new(NativeArtifact::for_filename(filename))),
                root: None,
                revision: Rc::new(()),
                failure: None,
            }));
            self.entries
                .borrow_mut()
                .insert(filename.into(), Rc::clone(&entry));
            entry
        });
        let (controller, root) = {
            let entry = entry.borrow();
            (Rc::clone(&entry.controller), entry.root.clone())
        };
        let changed = controller.borrow().content() != content;
        {
            let mut artifact = controller
                .try_borrow_mut()
                .map_err(|_| ToolUiError::Presentation("Artifact is being updated".into()))?;
            artifact.set_filename(filename.into());
            if let NativeArtifact::Html(html) = &mut *artifact {
                html.runtime_providers = panel.get_html_artifact_runtime_providers();
                html.sandbox_url_provider
                    .clone_from(&self.options.url.borrow());
            }
            if changed {
                entry.borrow_mut().revision = Rc::new(());
            }
            artifact.set_content(content.into())?;
        }
        if let Some(root) = root {
            super::super::native::update(&controller, &root, &self.view.borrow())?;
        }
        Ok(())
    }
    /// Remove only the named native view; selection fallback belongs to application commands.
    fn remove(&self, filename: &str) {
        let removed = self.entries.borrow_mut().shift_remove(filename);
        if let Some(entry) = removed {
            let root = entry.borrow().root.clone();
            if let Some(root) = root {
                root.node.unchecked_ref::<web_sys::Element>().remove();
            }
        }
    }
    /// Reload only a live HTML owner after rebinding the application-provided globals.
    fn reload_html(&self, entry: &RefCell<Entry>, panel: &ArtifactsPanelState) {
        let (controller, root) = {
            let entry = entry.borrow();
            (Rc::clone(&entry.controller), entry.root.clone())
        };
        let mut artifact = controller.borrow_mut();
        let NativeArtifact::Html(html) = &mut *artifact else {
            return;
        };
        html.runtime_providers = panel.get_html_artifact_runtime_providers();
        html.sandbox_url_provider
            .clone_from(&self.options.url.borrow());
        drop(artifact);
        let result = root.as_ref().map_or(Ok(()), |root| {
            super::super::native::update(&controller, root, &self.view.borrow())
        });
        let result = result.and_then(|()| match &*controller.borrow() {
            NativeArtifact::Html(html) => html.execute_content(html.content()),
            _ => Ok(()),
        });
        entry.borrow_mut().failure = result.err().map(|error| error.to_string());
    }
}
impl ArtifactPresentation for Presentation {
    fn now(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::from_timestamp_millis(js_sys::Date::now().to_i64().unwrap_or_default())
            .unwrap_or(chrono::DateTime::UNIX_EPOCH)
    }
    fn get_or_create_artifact_element(
        &self,
        filename: &str,
        content: &str,
        panel: ArtifactsPanelState,
    ) {
        if let Some(state) = self.0.upgrade() {
            let failure = state
                .upsert(filename, content, &panel)
                .err()
                .map(|error| error.to_string());
            if let Some(entry) = state.entries.borrow().get(filename).cloned() {
                entry.borrow_mut().failure = failure;
            }
        }
    }
    fn show_artifact(&self, filename: &str) {
        if let Some(state) = self.0.upgrade() {
            *state.scroll.borrow_mut() = Some(filename.into());
        }
    }
    fn remove_artifact_element(&self, filename: &str) {
        if let Some(state) = self.0.upgrade() {
            state.remove(filename);
        }
    }
    fn reload_all_html_artifacts(&self, panel: ArtifactsPanelState) {
        let Some(state) = self.0.upgrade() else {
            return;
        };
        for (_, entry) in state.entries() {
            state.reload_html(&entry, &panel);
        }
    }
    fn get_logs(&self, filename: &str) -> Result<ArtifactLogReader, ArtifactLogError> {
        let state = self.0.upgrade().ok_or(ArtifactLogError::Missing)?;
        let entry = state
            .entries
            .borrow()
            .get(filename)
            .cloned()
            .ok_or(ArtifactLogError::Missing)?;
        let controller = Rc::clone(&entry.borrow().controller);
        if !matches!(*controller.borrow(), NativeArtifact::Html(_)) {
            return Err(ArtifactLogError::NotHtml);
        }
        let i18n = Rc::clone(&state.view.borrow().i18n);
        Ok(Rc::new(move || match &*controller.borrow() {
            NativeArtifact::Html(html) => html
                .get_logs(&i18n)
                .unwrap_or_else(|error| error.to_string()),
            _ => String::new(),
        }))
    }
    fn wait(&self, duration: Duration) -> BoxFuture<()> {
        Box::pin(wait(duration))
    }
    fn request_update(&self) {
        if let Some(state) = self.0.upgrade() {
            state.refresh();
        }
    }
    fn on_artifacts_change(&self) {
        let callback = self
            .0
            .upgrade()
            .and_then(|state| state.options.changed.borrow().clone());
        if let Some(callback) = callback {
            callback();
        }
    }
    fn on_open(&self) {
        let callback = self
            .0
            .upgrade()
            .and_then(|state| state.options.open.borrow().clone());
        if let Some(callback) = callback {
            callback();
        }
    }
}
/// Await the application's requested delay through the existing cancel-on-drop timer owner.
async fn wait(duration: Duration) {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let sender = RefCell::new(Some(sender));
    let Some(window) = web_sys::window() else {
        return;
    };
    let callback = Rc::new(move || {
        if let Some(sender) = sender.take() {
            let _ = sender.send(());
        }
    });
    let delay = i32::try_from(duration.as_millis()).unwrap_or(i32::MAX);
    match crate::browser::timer::native(window, delay, false, callback) {
        Ok(timer) => {
            let _ = receiver.await;
            drop(timer);
        }
        Err(error) => web_sys::console::error_1(&error),
    }
}
