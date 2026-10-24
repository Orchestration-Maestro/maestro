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
    /// Waits for the next nonempty editor submission, trimmed.
    ///
    /// The returned future owns its wait and does not borrow the mode. Its first poll
    /// makes it the only waiter: submissions made before then are not kept for it, and a
    /// later call's first poll replaces it, leaving it pending.
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

    /// Installs submission on the editor without keeping the mode alive: trimmed nonempty
    /// text runs a model command or queues as steering during a compaction or response;
    /// otherwise it goes to the waiting input, if any, then into history.
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

    /// Clears the editor and runs `/model` with `search` on the terminal runtime.
    fn submit_model_command(self: Rc<Self>, search: Option<String>) {
        self.editor.set_text("");
        let runtime = Rc::clone(&self.tool_environment.runtime);
        runtime.spawn_local(Box::pin(
            async move { self.handle_model_command(search).await },
        ));
    }
}
