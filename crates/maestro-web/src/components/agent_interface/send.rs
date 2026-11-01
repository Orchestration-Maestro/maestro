//! Send admission: draft check, key resolution, the pre-send hook, then the prompt.
use super::{
    browser::{AgentInterface, Session},
    session::stored_key,
};
use crate::{SharedAttachment, components::message_editor::can_send, tools::types::ToolUiError};
use maestro_app::browser::components::messages::{
    CustomAgentMessages as BrowserMessages, UserMessageWithAttachments,
};
use maestro_app::{AgentMessage, CustomAgentMessages, PromptInput, UserContent};
use std::{
    rc::Rc,
    sync::{Arc, PoisonError, RwLock},
};

impl<C: CustomAgentMessages + 'static> AgentInterface<C> {
    /// Send `input` with `attachments` as the editor would.
    ///
    /// A blank draft without attachments, or a send while a response streams, does nothing. A
    /// provider with neither a stored key nor a stored custom provider key of that name asks
    /// `on_api_key_required` first; a refusal, or no such callback, keeps the draft. The
    /// pre-send hook runs next, and its failure also keeps the draft. Only then is the draft
    /// cleared if it has not changed during admission, and the prompt started.
    /// # Errors
    /// Returns a missing session, a failed pre-send hook or the prompt's failure.
    pub async fn send_message(
        &self,
        input: &str,
        attachments: Vec<SharedAttachment>,
    ) -> Result<(), ToolUiError> {
        self.state.send(input, attachments).await
    }
}
impl<C: CustomAgentMessages + 'static> Session<C> {
    /// Admit and send one draft.
    pub(super) async fn send(
        self: &Rc<Self>,
        input: &str,
        attachments: Vec<SharedAttachment>,
    ) -> Result<(), ToolUiError> {
        let streaming = self.session().is_some_and(|session| {
            session
                .state()
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .is_streaming()
        });
        if !can_send(input, attachments.len(), false) || streaming {
            return Ok(());
        }
        let revision = self.editor.revision();
        let session = self
            .session()
            .ok_or_else(|| ToolUiError::Presentation("No session set on AgentInterface".into()))?;
        let provider = session
            .state()
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .model
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .provider
            .clone();
        if !self.has_key(&provider).await {
            return Ok(());
        }
        let hook = self.props.borrow().on_before_send.clone();
        if let Some(hook) = hook {
            hook()
                .await
                .map_err(|error| ToolUiError::Presentation(error.message))?;
        }
        if self.editor.revision() == revision {
            self.editor.set_value("")?;
            self.editor.set_attachments(Vec::new())?;
        }
        let mut follow = self.follow.get();
        follow.enabled = true;
        self.follow.set(follow);
        session
            .prompt(prompt(input, attachments))
            .await
            .map_err(|error| ToolUiError::Presentation(error.message))
    }
    /// Whether `provider` has a key: a stored provider key, the stored key of the custom provider
    /// of that name, or one the caller's prompt reports as stored.
    async fn has_key(&self, provider: &str) -> bool {
        if stored_key(provider).await.is_some() {
            return true;
        }
        let ask = self.props.borrow().on_api_key_required.clone();
        if let Some(ask) = ask {
            return ask(provider.to_owned()).await;
        }
        web_sys::console::error_1(
            &"No API key configured and no onApiKeyRequired handler set".into(),
        );
        false
    }
}
/// The prompt for a draft: text alone, or text with its original attachments.
fn prompt<C: CustomAgentMessages>(
    input: &str,
    attachments: Vec<SharedAttachment>,
) -> PromptInput<BrowserMessages<C>> {
    if attachments.is_empty() {
        return PromptInput::Text {
            text: input.to_owned(),
            images: Vec::new(),
        };
    }
    PromptInput::Message(AgentMessage::Custom(Arc::new(RwLock::new(
        BrowserMessages::UserWithAttachments(UserMessageWithAttachments {
            content: UserContent::Text(input.to_owned()),
            timestamp: js_sys::Date::now(),
            attachments: Some(attachments),
        }),
    ))))
}
