//! Opt-in confirmation before replacing or forking a session.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use maestro_extensions_wasm::{
    AgentMessage, ExtensionAPI, ExtensionContext, ExtensionEvent, ExtensionEventResult,
    ExtensionFuture, Message, NotificationType, Presence, SessionBeforeSwitchReason,
    SessionBeforeSwitchResult, SessionEntry, SessionEvent,
};

/// Asks before a new session or before leaving history containing a user message.
fn before_switch(
    event: &mut ExtensionEvent,
    ctx: ExtensionContext,
) -> ExtensionFuture<'_, Option<ExtensionEventResult>> {
    Box::pin(async move {
        if !ctx.has_ui()? {
            return Ok(None);
        }
        let ExtensionEvent::Session(SessionEvent::BeforeSwitch(event)) = event else {
            return Ok(None);
        };
        let (title, message, cancelled) = match event.reason {
            SessionBeforeSwitchReason::New => (
                "Clear session?",
                "This will delete all messages in the current session.",
                "Clear cancelled",
            ),
            SessionBeforeSwitchReason::Resume => {
                let has_user_message = ctx.session_manager()?.get_entries()?.iter().any(|entry| {
                    matches!(entry, SessionEntry::Message(entry)
                        if matches!(&entry.message, AgentMessage::Message(message)
                            if matches!(message.as_ref(), Message::User(_))))
                });
                if !has_user_message {
                    return Ok(None);
                }
                (
                    "Switch session?",
                    "You have messages in the current session. Switch anyway?",
                    "Switch cancelled",
                )
            }
        };
        if ctx
            .ui()?
            .confirm(title.into(), message.into(), None)
            .await?
        {
            return Ok(None);
        }
        ctx.ui()?.notify(cancelled, Some(NotificationType::Info))?;
        Ok(Some(ExtensionEventResult::SessionBeforeSwitch(
            SessionBeforeSwitchResult {
                cancel: Presence::Present(true),
            },
        )))
    })
}

/// Registers session confirmation handlers with the supplied host.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move { api.on("session_before_switch", Rc::new(before_switch)) })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
