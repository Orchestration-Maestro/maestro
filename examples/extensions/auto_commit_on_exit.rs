//! Opt-in commit of a changed Git working tree when the session shuts down, titled with the
//! first line of the newest assistant reply.
// maestro-component: MAESTRO_AUTO_COMMIT_ON_EXIT_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use maestro_extensions_wasm::{
    AgentMessage, AssistantContent, ExtensionAPI, ExtensionContext, ExtensionEventResult,
    ExtensionFuture, ExtensionResult, Message, NotificationType, SessionEntry,
};

/// Characters of the reply line kept before the ellipsis.
const TITLE_LIMIT: usize = 50;

/// Title used when the newest reply supplies no first line.
const FALLBACK_TITLE: &str = "Session changes";

/// The first line of the newest assistant reply's first text block, empty without one.
fn reply_line(ctx: &ExtensionContext) -> ExtensionResult<String> {
    for entry in ctx.session_manager()?.get_entries()?.into_iter().rev() {
        if let SessionEntry::Message(entry) = entry
            && let AgentMessage::Message(message) = entry.message
            && let Message::Assistant(message) = *message
        {
            let text = message.content.into_iter().find_map(|part| match part {
                AssistantContent::Text(text) => Some(text.text),
                _ => None,
            });
            return Ok(text
                .and_then(|text| text.split('\n').next().map(str::to_owned))
                .unwrap_or_default());
        }
    }
    Ok(String::new())
}

/// Builds the commit message, cutting the line by characters so none is split.
fn commit_message(line: &str) -> String {
    let line = if line.is_empty() {
        FALLBACK_TITLE
    } else {
        line
    };
    let mut title: String = line.chars().take(TITLE_LIMIT).collect();
    if title.len() < line.len() {
        title.push_str("...");
    }
    format!("[maestro] {title}")
}

/// Stages and commits a changed working tree, announcing only a successful commit.
fn commit_on_exit(
    api: ExtensionAPI,
    ctx: ExtensionContext,
) -> ExtensionFuture<'static, Option<ExtensionEventResult>> {
    Box::pin(async move {
        let status = api
            .exec("git", &["status".into(), "--porcelain".into()], None)
            .await?;
        // Every porcelain line carries a status code, so any whitespace set gives the same answer.
        if status.code != 0 || status.stdout.trim().is_empty() {
            return Ok(None);
        }
        let message = commit_message(&reply_line(&ctx)?);
        api.exec("git", &["add".into(), "-A".into()], None).await?;
        let commit = api
            .exec(
                "git",
                &["commit".into(), "-m".into(), message.clone()],
                None,
            )
            .await?;
        if commit.code == 0 && ctx.has_ui()? {
            ctx.ui()?.notify(
                &format!("Committed on exit: {message}"),
                Some(NotificationType::Info),
            )?;
        }
        Ok(None)
    })
}

/// Registers the shutdown hook through the supplied host.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let registered = api.clone();
        api.on(
            "session_shutdown",
            Rc::new(move |_, ctx| commit_on_exit(registered.clone(), ctx)),
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
