//! Opt-in confirmation before replacing or forking a session.
// maestro-component: MAESTRO_CONFIRM_DESTRUCTIVE_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use maestro_extensions_wasm::{
    AgentMessage, ExtensionAPI, ExtensionContext, ExtensionEvent, ExtensionEventResult,
    ExtensionFuture, Message, NotificationType, Presence, SessionBeforeForkResult,
    SessionBeforeSwitchReason, SessionBeforeSwitchResult, SessionEntry, SessionEvent,
};

/// Asks before a new session or before leaving history containing a user message.
///
/// # Errors
/// The returned future returns a UI availability, session-entry, confirmation or
/// notification failure.
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

/// Permits a fork only when its selector returns the exact affirmative choice.
///
/// # Errors
/// The returned future returns a UI availability, selection or notification failure.
fn before_fork(
    event: &mut ExtensionEvent,
    ctx: ExtensionContext,
) -> ExtensionFuture<'_, Option<ExtensionEventResult>> {
    Box::pin(async move {
        if !ctx.has_ui()? {
            return Ok(None);
        }
        let ExtensionEvent::Session(SessionEvent::BeforeFork(event)) = event else {
            return Ok(None);
        };
        let entry_id: String = event.entry_id.chars().take(8).collect();
        let choice = ctx
            .ui()?
            .select(
                format!("Fork from entry {entry_id}?"),
                vec![
                    "Yes, create fork".into(),
                    "No, stay in current session".into(),
                ],
                None,
            )
            .await?;
        if choice.as_deref() == Some("Yes, create fork") {
            return Ok(None);
        }
        ctx.ui()?
            .notify("Fork cancelled", Some(NotificationType::Info))?;
        Ok(Some(ExtensionEventResult::SessionBeforeFork(
            SessionBeforeForkResult {
                cancel: Presence::Present(true),
                skip_conversation_restore: Presence::Missing,
            },
        )))
    })
}

/// Registers session confirmation handlers with the supplied host.
///
/// # Errors
/// The returned future returns the host's rejection when it refuses a handler
/// registration.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        api.on("session_before_switch", Rc::new(before_switch))?;
        api.on("session_before_fork", Rc::new(before_fork))
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
