//! A dialog asking for a provider's key, settled once a key is stored or the dialog closes.
use super::{
    dialog::{DialogHeaderProps, dialog_content, dialog_header},
    dialog_base::DialogBase,
};
use crate::{
    browser::timer::{Timer, native},
    components::provider_key_input::ProviderKeyInput,
    dom::{DomContent, DomRoot, DomView},
    tools::types::ToolUiError,
};
use maestro_app::get_app_storage;
use std::{
    any::Any,
    cell::RefCell,
    rc::{Rc, Weak},
};
use tokio::sync::oneshot;

/// Panel width of the key prompt.
const WIDTH: &str = "min(500px, 90vw)";
/// How often stored keys are checked, in milliseconds.
const CHECK_MS: i32 = 500;
/// Retained state of one open key prompt.
struct Prompt {
    /// The shown dialog.
    base: DialogBase,
    /// Mounted key entry.
    _entry: DomRoot,
    /// Settlement, taken by the first outcome.
    settle: RefCell<Option<oneshot::Sender<bool>>>,
    /// Stored-key observation, released when the dialog closes.
    check: RefCell<Option<Timer>>,
}
impl Prompt {
    /// Deliver `stored` unless an earlier outcome was delivered.
    fn finish(&self, stored: bool) {
        self.check.take();
        if let Some(settle) = self.settle.borrow_mut().take() {
            let _ = settle.send(stored);
        }
    }
}
/// A prompt for a missing provider key.
pub struct ApiKeyPromptDialog;
impl ApiKeyPromptDialog {
    /// Show the prompt for `provider`'s key now, resolving to `true` once a key is stored and
    /// `false` when the dialog closes first.
    /// # Errors
    /// Returns a localization, timer or native failure; nothing is shown then.
    pub fn prompt(
        view: &Rc<DomView>,
        provider: &str,
    ) -> Result<impl Future<Output = bool> + use<>, ToolUiError> {
        let receiver = Self::show(view, provider)?;
        Ok(async move { receiver.await.unwrap_or(false) })
    }
    /// Open the prompt and start observing stored keys.
    fn show(view: &Rc<DomView>, provider: &str) -> Result<oneshot::Receiver<bool>, ToolUiError> {
        let document = &view.document;
        let entry = ProviderKeyInput::new(provider).mount(view)?;
        let header = dialog_header(
            document,
            &DialogHeaderProps {
                title: view.i18n.text("API Key Required")?,
                ..DialogHeaderProps::default()
            },
        )?;
        let content = dialog_content(document, &[header.into(), entry.node.clone()], "")?;
        let (sender, receiver) = oneshot::channel();
        let prompt = Rc::new(Prompt {
            base: DialogBase::new(Rc::clone(view), WIDTH, "auto"),
            _entry: entry,
            settle: RefCell::new(Some(sender)),
            check: RefCell::default(),
        });
        let weak = Rc::downgrade(&prompt);
        prompt.base.set_on_close(Some(Rc::new(move || {
            if let Some(prompt) = weak.upgrade() {
                prompt.finish(false);
            }
        })));
        let component: Rc<dyn Any> = Rc::clone(&prompt) as Rc<dyn Any>;
        prompt.base.open(vec![content.into()], Some(component))?;
        let window = document
            .default_view()
            .ok_or_else(|| ToolUiError::Presentation("Document has no window".into()))?;
        let observe = check(Rc::downgrade(&prompt), provider.to_owned());
        let timer = native(window, CHECK_MS, true, observe)
            .map_err(|error| crate::browser::attachment_content::browser::host_error(&error));
        match timer {
            Ok(timer) => {
                prompt.check.replace(Some(timer));
                Ok(receiver)
            }
            Err(error) => {
                prompt.base.close();
                Err(error.into())
            }
        }
    }
}
/// One stored-key check: a stored key settles the prompt as stored and closes it.
fn check(prompt: Weak<Prompt>, provider: String) -> Rc<dyn Fn()> {
    Rc::new(move || {
        let prompt = prompt.clone();
        let provider = provider.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let stored = match get_app_storage() {
                Ok(storage) => storage.provider_keys.get(&provider).await,
                Err(error) => Err(error),
            };
            let has_key = matches!(stored, Ok(Some(key)) if !key.is_empty());
            if let Some(prompt) = prompt.upgrade().filter(|_| has_key) {
                prompt.finish(true);
                prompt.base.close();
            }
        });
    })
}
