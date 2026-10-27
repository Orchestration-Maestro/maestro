//! Summarizes the complete prepared conversation with a separately selected model.
#![cfg(target_arch = "wasm32")]
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use maestro_extensions_wasm::{
    AssistantContent, CompactionResult, Context, ExtensionAPI, ExtensionContext, ExtensionEvent,
    ExtensionEventResult, ExtensionFuture, ExtensionResult, Message, NotificationType, Presence,
    ProviderStreamOptions, ResolvedRequestAuth, SessionBeforeCompactEvent,
    SessionBeforeCompactResult, SessionEvent, StreamOptions, TextContent, UserBlock, UserContent,
    UserMessage, complete, convert_to_llm, serialize_conversation,
};
use std::{
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
};

/// Builds the summarizer's instructions without losing the earlier summary.
fn prompt(conversation: &str, previous: &Presence<String>) -> String {
    let previous = match previous {
        Presence::Present(summary) if !summary.is_empty() => {
            format!("\n\nPrevious session summary for context:\n{summary}")
        }
        _ => String::new(),
    };
    format!("You are a conversation summarizer. Create a comprehensive summary of this conversation that captures:{previous}

1. The main goals and objectives discussed
2. Key decisions made and their rationale
3. Important code changes, file modifications, or technical details
4. Current state of any ongoing work
5. Any blockers, issues, or open questions
6. Next steps that were planned or suggested

Be thorough but concise. The summary will replace the ENTIRE conversation history, so include all information needed to continue the work effectively.

Format the summary as structured markdown with clear sections.

<conversation>
{conversation}
</conversation>")
}

/// Supplies one ordinary user request with the current host wall-clock timestamp.
fn summary_context(text: String) -> ExtensionResult<Context> {
    let timestamp = (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_secs_f64()
        * 1000.0)
        .floor();
    Ok(Context {
        system_prompt: None,
        messages: vec![Message::User(UserMessage {
            content: UserContent::Blocks(vec![UserBlock::Text(TextContent::new(text))]),
            timestamp,
        })],
        tools: None,
    })
}

/// Joins text blocks in response order, leaving other content out of the summary.
fn summary_text(content: &[AssistantContent]) -> String {
    content
        .iter()
        .filter_map(|part| match part {
            AssistantContent::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Requests a summary using the host's model, authentication and conversation helpers.
async fn summarize(
    api: &ExtensionAPI,
    event: &SessionBeforeCompactEvent,
    ctx: &ExtensionContext,
) -> ExtensionResult<CompactionResult> {
    let registry = ctx.model_registry()?;
    let model = registry
        .find("google", "gemini-2.5-flash")?
        .ok_or("Could not find Gemini Flash model, using default compaction")?;
    let ResolvedRequestAuth::Ok {
        api_key: Some(api_key),
        headers,
    } = registry.get_api_key_and_headers(model.clone()).await?
    else {
        return Err("No API key for google, using default compaction".into());
    };
    let preparation = &event.preparation;
    let messages: Vec<_> = preparation
        .messages_to_summarize
        .iter()
        .chain(&preparation.turn_prefix_messages)
        .cloned()
        .collect();
    ctx.ui()?.notify(
        &format!(
            "Custom compaction: summarizing {} messages ({} tokens) with {}...",
            messages.len(),
            preparation.tokens_before,
            model.id,
        ),
        Some(NotificationType::Info),
    )?;
    let conversation = serialize_conversation(api, &convert_to_llm(api, &messages)?)?;
    let response = complete(
        model,
        summary_context(prompt(&conversation, &preparation.previous_summary))?,
        Some(ProviderStreamOptions {
            common: StreamOptions {
                api_key: Some(api_key),
                headers,
                max_tokens: Some(8192.0),
                signal: Some(event.signal.clone()),
                ..Default::default()
            },
            ..Default::default()
        }),
    )
    .await?;
    let response = response.read().map_err(|error| error.to_string())?;
    let summary = summary_text(&response.content);
    Ok(CompactionResult {
        summary,
        first_kept_entry_id: preparation.first_kept_entry_id.clone(),
        tokens_before: preparation.tokens_before,
        details: Presence::Missing,
    })
}

/// Handles the supplied compaction without changing session history directly.
async fn on_compact(
    api: ExtensionAPI,
    event: &mut ExtensionEvent,
    ctx: ExtensionContext,
) -> ExtensionResult<Option<ExtensionEventResult>> {
    let ExtensionEvent::Session(SessionEvent::BeforeCompact(event)) = event else {
        return Ok(None);
    };
    ctx.ui()?.notify(
        "Custom compaction extension triggered",
        Some(NotificationType::Info),
    )?;
    let compaction = summarize(&api, event, &ctx).await?;
    Ok(Some(ExtensionEventResult::SessionBeforeCompact(
        SessionBeforeCompactResult {
            cancel: Presence::Missing,
            compaction: Presence::Present(compaction),
        },
    )))
}

/// Registers custom compaction for this explicitly loaded extension.
#[must_use]
pub fn custom_compaction(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let helpers = api.clone();
        api.on(
            "session_before_compact",
            Rc::new(move |event, ctx| Box::pin(on_compact(helpers.clone(), event, ctx))),
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(custom_compaction);
