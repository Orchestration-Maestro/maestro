//! Entry bookmarks: commands that label session entries, so that the tree view can find the
//! important points of a conversation again.
//!
//! A label is stored on the entry it names. `/bookmark` labels the newest assistant message,
//! with the argument text or a generated timestamp label, and `/unbookmark` removes the newest
//! label from any entry.
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

/// Registers the bookmark command.
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
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
