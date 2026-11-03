//! Entry bookmarks: commands that label session entries, so that the tree view can find the
//! important points of a conversation again.
//!
//! A label is stored on the entry it names. `/bookmark` labels the newest assistant message,
//! with the argument text or a generated timestamp label, and `/unbookmark` removes the newest
//! label from any entry.
// maestro-component: MAESTRO_BOOKMARK_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use maestro_extensions_wasm::{
    AgentMessage, CommandOptions, ExtensionAPI, ExtensionCommandContext, ExtensionFuture,
    ExtensionResult, Message, NotificationType, SessionEntry,
};

/// Labels the newest assistant message through the supplied host.
fn bookmark(api: &ExtensionAPI, args: &str, ctx: &ExtensionCommandContext) -> ExtensionResult<()> {
    let trimmed = args.trim();
    let label = if trimmed.is_empty() {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?;
        format!("bookmark-{}", elapsed.as_millis())
    } else {
        trimmed.to_owned()
    };
    for entry in ctx.session_manager()?.get_entries()?.iter().rev() {
        if let SessionEntry::Message(message) = entry
            && matches!(&message.message, AgentMessage::Message(value)
                if matches!(value.as_ref(), Message::Assistant(_)))
        {
            api.set_label(&message.base.id, Some(&label))?;
            return ctx.ui()?.notify(
                &format!("Bookmarked as: {label}"),
                Some(NotificationType::Info),
            );
        }
    }
    ctx.ui()?.notify(
        "No assistant message to bookmark",
        Some(NotificationType::Warning),
    )
}

/// Reads the identity of a session entry, whichever of the nine kinds it is.
fn entry_id(entry: &SessionEntry) -> &str {
    match entry {
        SessionEntry::Message(value) => &value.base.id,
        SessionEntry::ThinkingLevelChange(value) => &value.base.id,
        SessionEntry::ModelChange(value) => &value.base.id,
        SessionEntry::Compaction(value) => &value.base.id,
        SessionEntry::BranchSummary(value) => &value.base.id,
        SessionEntry::Custom(value) => &value.base.id,
        SessionEntry::CustomMessage(value) => &value.base.id,
        SessionEntry::Label(value) => &value.base.id,
        SessionEntry::SessionInfo(value) => &value.base.id,
    }
}

/// Removes the newest label through the supplied host.
fn unbookmark(api: &ExtensionAPI, ctx: &ExtensionCommandContext) -> ExtensionResult<()> {
    let reader = ctx.session_manager()?;
    for entry in reader.get_entries()?.iter().rev() {
        let id = entry_id(entry);
        if let Some(label) = reader.get_label(id)?.filter(|value| !value.is_empty()) {
            api.set_label(id, None)?;
            return ctx.ui()?.notify(
                &format!("Removed bookmark: {label}"),
                Some(NotificationType::Info),
            );
        }
    }
    ctx.ui()?
        .notify("No bookmarked entry found", Some(NotificationType::Warning))
}

/// Registers the bookmark and removal commands.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let registered = api.clone();
        api.register_command(
            "bookmark",
            CommandOptions {
                description: Some("Bookmark last message (usage: /bookmark [label])".into()),
                get_argument_completions: None,
                handler: Rc::new(move |args, ctx| {
                    let api = registered.clone();
                    Box::pin(async move { bookmark(&api, &args, &ctx) })
                }),
            },
        )?;
        let registered = api.clone();
        api.register_command(
            "unbookmark",
            CommandOptions {
                description: Some("Remove bookmark from last labeled entry".into()),
                get_argument_completions: None,
                handler: Rc::new(move |_, ctx| {
                    let api = registered.clone();
                    Box::pin(async move { unbookmark(&api, &ctx) })
                }),
            },
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
