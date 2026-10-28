//! The working row shown while a response runs, and the extension controls of that row and
//! of the hidden-thinking label.

use super::InteractiveMode;
use crate::{AssistantMessageComponent, AssistantMessageOptions};
use maestro_app::WorkingIndicatorOptions;
use maestro_theme::{ThemeColor, paint};
use maestro_tui::{Loader, LoaderIndicatorOptions};
use std::{
    any::Any,
    cell::{Cell, RefCell},
    rc::Rc,
};

/// The working message shown when no extension set one.
const DEFAULT_WORKING_MESSAGE: &str = "Working...";

/// The working row and its extension-controlled presentation.
pub(super) struct WorkingState {
    /// The working row while a response runs.
    pub(super) loading_animation: RefCell<Option<Loader>>,
    /// The extension's working message, the default one when absent.
    pub(super) working_message: RefCell<Option<String>>,
    /// Whether the working row is shown while a response runs.
    pub(super) working_visible: Cell<bool>,
    /// The extension's working indicator, the toolkit's own when absent.
    pub(super) working_indicator_options: RefCell<Option<WorkingIndicatorOptions>>,
    /// The label shown for hidden thinking.
    pub(super) hidden_thinking_label: RefCell<String>,
}

impl Default for WorkingState {
    fn default() -> Self {
        Self {
            loading_animation: RefCell::default(),
            working_message: RefCell::default(),
            working_visible: Cell::new(true),
            working_indicator_options: RefCell::default(),
            hidden_thinking_label: RefCell::new(
                AssistantMessageOptions::default().hidden_thinking_label,
            ),
        }
    }
}

impl InteractiveMode {
    /// The extension's working message, or the default one.
    fn get_working_loader_message(&self) -> String {
        let message = self.working.working_message.borrow().clone();
        message.unwrap_or_else(|| DEFAULT_WORKING_MESSAGE.to_owned())
    }

    /// Builds the working row: the extension's indicator, or the toolkit's own in the accent
    /// colour, beside the muted working message, both painted by the theme published when
    /// the loader last updated its frame or message.
    pub(super) fn create_working_loader(&self) -> Loader {
        let color = |role: ThemeColor| -> Rc<dyn Fn(&str) -> String> {
            let theme = self.themes.theme();
            Rc::new(move |text| paint(theme.get().ok().as_deref(), &role, text))
        };
        let indicator = self.working.working_indicator_options.borrow().clone();
        Loader::new(
            self.ui.as_ref().clone(),
            color(ThemeColor::Accent),
            color(ThemeColor::Muted),
            Some(self.get_working_loader_message()),
            indicator.map(loader_indicator),
        )
    }

    /// Builds the working row and shows it in the status rows.
    pub(super) fn show_working_loader(&self) {
        let loader = self.create_working_loader();
        self.status_container.add_child(Rc::new(loader.clone()));
        drop(self.working.loading_animation.replace(Some(loader)));
    }

    /// Stops and releases the working row without changing the status rows.
    pub(super) fn release_working_loader(&self) {
        if let Some(loader) = self.working.loading_animation.take() {
            loader.stop();
        }
    }

    /// Stops and releases the working row, then empties the status rows.
    pub(super) fn stop_working_loader(&self) {
        self.release_working_loader();
        self.status_container.clear();
    }

    /// Sets the working message, `None` restoring the default, and shows it in a running
    /// working row.
    pub(super) fn set_working_message(&self, message: Option<String>) {
        self.working.working_message.replace(message);
        let loader = self.working.loading_animation.borrow().clone();
        if let Some(loader) = loader {
            loader.set_message(self.get_working_loader_message());
        }
    }

    /// Shows or hides the working row: hiding stops it, and showing while a response
    /// streams without one starts it.
    pub(super) fn set_working_visible(&self, visible: bool) {
        self.working.working_visible.set(visible);
        if !visible {
            self.stop_working_loader();
        } else if self.runtime_host.session().is_streaming()
            && self.working.loading_animation.borrow().is_none()
        {
            self.status_container.clear();
            self.show_working_loader();
        }
        self.ui.request_render(false);
    }

    /// Sets the working indicator, `None` restoring the toolkit's own, and applies it to a
    /// running working row.
    pub(super) fn set_working_indicator(&self, options: Option<WorkingIndicatorOptions>) {
        self.working
            .working_indicator_options
            .replace(options.clone());
        let loader = self.working.loading_animation.borrow().clone();
        if let Some(loader) = loader {
            loader.set_indicator(options.map(loader_indicator));
        }
        self.ui.request_render(false);
    }

    /// Restores the default working message, visibility and indicator, shows the default
    /// message with the interrupt key on a running working row, then restores the default
    /// hidden-thinking label.
    pub(super) fn reset_working(&self) {
        drop(self.working.working_message.take());
        self.working.working_visible.set(true);
        self.set_working_indicator(None);
        let loader = self.working.loading_animation.borrow().clone();
        if let Some(loader) = loader {
            let interrupt = maestro_app::key_text("app.interrupt");
            loader.set_message(format!(
                "{DEFAULT_WORKING_MESSAGE} ({interrupt} to interrupt)"
            ));
        }
        self.set_hidden_thinking_label(None);
    }

    /// Sets the hidden-thinking label, `None` restoring the default, on every assistant row
    /// of the transcript and on the streaming row; later rows start with it.
    pub(super) fn set_hidden_thinking_label(&self, label: Option<String>) {
        let label =
            label.unwrap_or_else(|| AssistantMessageOptions::default().hidden_thinking_label);
        self.working.hidden_thinking_label.replace(label.clone());
        for child in self.chat_container.walk() {
            let child: &dyn Any = child.as_ref();
            if let Some(row) = child.downcast_ref::<AssistantMessageComponent>() {
                row.set_hidden_thinking_label(label.clone());
            }
        }
        if let Some(component) = self.streaming_component() {
            component.set_hidden_thinking_label(label);
        }
        self.ui.request_render(false);
    }
}

/// The toolkit form of an extension's working indicator.
fn loader_indicator(options: WorkingIndicatorOptions) -> LoaderIndicatorOptions {
    LoaderIndicatorOptions {
        frames: options.frames,
        interval_ms: options.interval_ms,
    }
}
