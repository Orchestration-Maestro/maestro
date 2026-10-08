//! The compaction example: a handler that summarizes the user requests, written against the
//! author facade only.
use std::rc::Rc;

use maestro_extensions_wasm::{
    AgentMessage, CompactionResult, ExtensionAPI, ExtensionEvent, ExtensionEventResult,
    SessionBeforeCompactResult, SessionEvent, UserContent,
};
use serde_json::json;

/// The request lines of the compaction example: every user message, each cut to one hundred
/// characters, or `[complex]` when it is not plain text.
#[must_use]
pub fn user_requests(messages: &[AgentMessage]) -> String {
    let lines: Vec<String> = messages
        .iter()
        .filter_map(|message| match message {
            AgentMessage::User(user) => Some(user),
            _ => None,
        })
        .map(|user| match &user.content {
            UserContent::Text(text) => format!("- {}", text.chars().take(100).collect::<String>()),
            UserContent::Blocks(_) => "- [complex]".to_owned(),
        })
        .collect();
    format!("User requests:\n{}", lines.join("\n"))
}

/// Registers the compaction example: it reads the session and the catalog through its
/// context, then answers with the user requests as the summary.
///
/// # Errors
/// Returns the host's message when a registration is rejected.
pub fn register_compaction_example(api: &ExtensionAPI) -> Result<(), String> {
    api.on(
        "session_before_compact",
        Rc::new(|event, ctx| {
            Box::pin(async move {
                let ExtensionEvent::Session(SessionEvent::BeforeCompact(compact)) = event else {
                    return Ok(None);
                };
                let entries = ctx.session_manager()?.get_entries()?;
                let probe = ctx.model_registry()?.find("provider", "model")?;
                let preparation = &compact.preparation;
                let summary = user_requests(&preparation.messages_to_summarize);
                Ok(Some(ExtensionEventResult::SessionBeforeCompact(SessionBeforeCompactResult {
                    cancel: None,
                    compaction: Some(CompactionResult {
                        summary,
                        first_kept_entry_id: preparation.first_kept_entry_id.clone(),
                        tokens_before: preparation.tokens_before,
                        details: Some(json!({ "entries": entries.len(), "model": probe.map(|model| model.id) }).to_string()),
                    }),
                })))
            })
        }),
    )?;
    api.on(
        "session_compact",
        Rc::new(|event, _ctx| {
            Box::pin(async move {
                let ExtensionEvent::Session(SessionEvent::Compact(compact)) = event else {
                    return Ok(None);
                };
                let entry = &compact.compaction_entry;
                Err(format!(
                    "{} {} {}",
                    entry.summary, entry.tokens_before, compact.from_extension
                ))
            })
        }),
    )
}
