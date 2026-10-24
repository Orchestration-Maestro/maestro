//! Editor submission and the single waiting input operation.
use super::InteractiveMode;
use maestro_app::is_whitespace_scalar;
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

    /// Installs submission on the editor without keeping the mode alive: a recognized
    /// command runs and reports its own outcome, and any other trimmed nonempty text goes
    /// to the waiting input, if any, then into the editor's history.
    ///
    /// The editor empties itself before calling back, so a command needs no clearing step.
    pub(super) fn setup_editor_submit_handler(self: &Rc<Self>) {
        let mode = Rc::downgrade(self);
        self.editor.set_on_submit(Some(Rc::new(move |text| {
            let text = text.trim_matches(is_whitespace_scalar);
            let Some(mode) = mode.upgrade().filter(|_| !text.is_empty()) else {
                return;
            };
            if mode.runs_command(text) {
                return;
            }
            let waiting = mode.on_input_callback.borrow_mut().take();
            if let Some(waiting) = waiting {
                // A dropped waiter no longer wants the text; history still records it.
                drop(waiting.send(text.to_owned()));
            }
            mode.editor.add_to_history(text);
        })));
    }

    /// Runs a recognized command, handing a failure beyond the transcript to the terminal
    /// runtime, and says whether `text` was handled as a command. A share or copy command
    /// is admitted on that runtime and clears the editor once it settles.
    fn runs_command(self: &Rc<Self>, text: &str) -> bool {
        if text == "/export" || text.starts_with("/export ") {
            if let Err(error) = self.handle_export_command(text) {
                self.report_runtime_error(error);
            }
            return true;
        }
        let share = text == "/share";
        if !share && text != "/copy" {
            return false;
        }
        let mode = Rc::clone(self);
        self.tool_environment
            .runtime
            .spawn_local(Box::pin(async move {
                let settled = if share {
                    mode.handle_share_command().await
                } else {
                    mode.handle_copy_command().await
                };
                mode.clear_editor();
                settled
            }));
        true
    }
}
