//! Editor submission and the single waiting input operation.
use super::InteractiveMode;
use maestro_app::is_whitespace_scalar;
use maestro_app::messages::UserMessageDelivery;
use maestro_tui::EditorComponent;
use std::{
    future::{Future, pending},
    rc::Rc,
};
use tokio::sync::oneshot;

impl InteractiveMode {
    /// Waits for the next submitted editor text the mode handles as input, trimmed.
    ///
    /// The returned future owns its wait and does not borrow the mode. Its first poll
    /// makes it the only waiter: submissions made before then are not kept for it, and a
    /// later call's first poll replaces it, leaving it pending. A submission the mode
    /// handles as a command is not delivered.
    pub fn get_user_input(&self) -> impl Future<Output = String> + use<> {
        let slot = Rc::clone(&self.on_input_callback);
        async move {
            let (sender, receiver) = oneshot::channel();
            let displaced = slot.replace(Some(sender));
            drop(displaced);
            match receiver.await {
                Ok(text) => text,
                Err(_) => pending().await,
            }
        }
    }

    /// Installs submission on the editor without keeping the mode alive: a `/scoped-models`
    /// or `/model` command clears the editor and runs itself, another recognized command
    /// runs and reports its own outcome, any other trimmed nonempty text is queued as
    /// steering during a compaction or a response, and otherwise it goes to the waiting
    /// input, if any, then into the editor's history.
    ///
    /// The editor empties itself before calling back, so a command needs no clearing step.
    pub(super) fn setup_editor_submit_handler(self: &Rc<Self>) {
        let mode = Rc::downgrade(self);
        self.editor.set_on_submit(Some(Rc::new(move |text| {
            let text = text.trim_matches(is_whitespace_scalar);
            let Some(mode) = mode.upgrade().filter(|_| !text.is_empty()) else {
                return;
            };
            if text == "/scoped-models" {
                mode.editor.set_text("");
                mode.report(mode.show_models_selector());
                return;
            }
            if text == "/model" || text.starts_with("/model ") {
                let search = text["/model".len()..].trim_matches(is_whitespace_scalar);
                mode.submit_model_command(
                    Some(search.to_owned()).filter(|search| !search.is_empty()),
                );
                return;
            }
            if mode.runs_command(text) {
                return;
            }
            match mode.submit_queued_input(text, UserMessageDelivery::Steer) {
                Ok(false) => {}
                queued => return mode.report(queued.map(drop)),
            }
            let waiting = mode.on_input_callback.borrow_mut().take();
            if let Some(waiting) = waiting {
                // A dropped waiter no longer wants the text; history still records it.
                drop(waiting.send(text.to_owned()));
            }
            mode.editor.add_to_history(text);
        })));
    }

    /// Runs the export command when `text` is one, handing a failure beyond the transcript
    /// to the terminal runtime, and says whether `text` was handled as a command.
    fn runs_command(&self, text: &str) -> bool {
        if text != "/export" && !text.starts_with("/export ") {
            return false;
        }
        if let Err(error) = self.handle_export_command(text) {
            self.report(Err(error));
        }
        true
    }

    /// Clears the editor and runs `/model` with `search` on the terminal runtime.
    fn submit_model_command(self: Rc<Self>, search: Option<String>) {
        self.editor.set_text("");
        let runtime = Rc::clone(&self.tool_environment.runtime);
        runtime.spawn_local(Box::pin(
            async move { self.handle_model_command(search).await },
        ));
    }
}
