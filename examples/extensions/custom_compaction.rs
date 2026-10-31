//! Summarizes the complete prepared conversation with a separately selected model.
// maestro-component: MAESTRO_CUSTOM_COMPACTION_COMPONENT
#![cfg(target_arch = "wasm32")]
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use maestro_extensions_wasm::{
    AbortSignal, AssistantContent, CompactionResult, Context, ExtensionAPI, ExtensionContext,
    ExtensionEvent, ExtensionEventResult, ExtensionFuture, ExtensionResult, Message, Model,
    NotificationType, Presence, ProviderStreamOptions, ResolvedRequestAuth,
    SessionBeforeCompactEvent, SessionBeforeCompactResult, SessionEvent, StopReason, StreamOptions,
    TextContent, UserBlock, UserContent, UserMessage, complete, convert_to_llm,
    serialize_conversation, utils,
};
use std::{
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
};

/// Builds the summarizer's instructions without losing the earlier summary.
fn prompt(conversation: &str, previous: &Presence<String>) -> String {
    let previous = match previous {
        Presence::Present(summary) if !summary.is_empty() => {
            format!("\n\nEarlier session context:\n{summary}")
        }
        _ => String::new(),
    };
    format!("Write a session handoff from the conversation below.{previous}

Cover these points:
- Goals the session is working toward.
- Decisions and the reasons behind them.
- Relevant code edits, affected files and technical details.
- Work in progress and its current status.
- Blockers, unresolved issues and unanswered questions.
- Planned or proposed actions to take next.

This summary replaces the full conversation history. Keep everything needed to resume the work, without unnecessary detail.

Use Markdown headings to organize the handoff.

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

/// Resolves the independently selected model without turning missing credentials into a request.
async fn model_options(
    ctx: &ExtensionContext,
    signal: AbortSignal,
) -> ExtensionResult<Option<(Model, ProviderStreamOptions)>> {
    let registry = ctx.model_registry()?;
    let Some(model) = registry.find("google", "gemini-2.5-flash")? else {
        ctx.ui()?.notify(
            "Summary model gemini-2.5-flash is unavailable; keeping default compaction",
            Some(NotificationType::Warning),
        )?;
        return Ok(None);
    };
    let (api_key, headers) = match registry.get_api_key_and_headers(model.clone()).await? {
        ResolvedRequestAuth::Ok { api_key, headers } => (api_key, headers),
        ResolvedRequestAuth::Error { error } => {
            ctx.ui()?.notify(
                &format!("Cannot authenticate the summary request: {error}"),
                Some(NotificationType::Warning),
            )?;
            return Ok(None);
        }
    };
    let Some(api_key) = api_key.filter(|key| !key.is_empty()) else {
        ctx.ui()?.notify(
            &format!(
                "Summary provider {} has no API key; keeping default compaction",
                model.provider
            ),
            Some(NotificationType::Warning),
        )?;
        return Ok(None);
    };
    Ok(Some((
        model,
        ProviderStreamOptions {
            common: StreamOptions {
                api_key: Some(api_key),
                headers,
                max_tokens: Some(8192.0),
                signal: Some(signal),
                ..Default::default()
            },
            ..Default::default()
        },
    )))
}

/// Tests emptiness with the same whitespace owner used by the host's other text operations.
fn summary_is_blank(api: &ExtensionAPI, text: &str) -> ExtensionResult<bool> {
    for ch in text.chars() {
        if !utils::is_whitespace_char(api, &ch.to_string())? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Requests a summary using the host's model, authentication and conversation helpers.
async fn summarize(
    api: &ExtensionAPI,
    event: &SessionBeforeCompactEvent,
    ctx: &ExtensionContext,
    model: Model,
    options: ProviderStreamOptions,
) -> ExtensionResult<Option<CompactionResult>> {
    let preparation = &event.preparation;
    let messages: Vec<_> = preparation
        .messages_to_summarize
        .iter()
        .chain(&preparation.turn_prefix_messages)
        .cloned()
        .collect();
    ctx.ui()?.notify(
        &format!(
            "Summarizing {} messages ({} tokens) with {}",
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
        Some(options),
    )
    .await?;
    let response = response.read().map_err(|error| error.to_string())?;
    if event.signal.aborted() || response.stop_reason == StopReason::Aborted {
        return Ok(None);
    }
    if response.stop_reason == StopReason::Error {
        return Err(response
            .error_message
            .as_deref()
            .unwrap_or("Summary provider failed")
            .into());
    }
    let summary = summary_text(&response.content);
    if summary_is_blank(api, &summary)? {
        ctx.ui()?.notify(
            "Summary contains no text; keeping default compaction",
            Some(NotificationType::Warning),
        )?;
        return Ok(None);
    }
    Ok(Some(CompactionResult {
        summary,
        first_kept_entry_id: preparation.first_kept_entry_id.clone(),
        tokens_before: preparation.tokens_before,
        details: Presence::Missing,
    }))
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
        "Custom summary requested for compaction",
        Some(NotificationType::Info),
    )?;
    if event.signal.aborted() {
        return Ok(None);
    }
    let Some((model, options)) = model_options(&ctx, event.signal.clone()).await? else {
        return Ok(None);
    };
    let compaction = match summarize(&api, event, &ctx, model, options).await {
        Ok(compaction) => compaction,
        Err(error) => {
            ctx.ui()?.notify(
                &format!("Cannot create the custom summary: {error}"),
                Some(NotificationType::Error),
            )?;
            None
        }
    };
    Ok(compaction.map(|compaction| {
        ExtensionEventResult::SessionBeforeCompact(SessionBeforeCompactResult {
            cancel: Presence::Missing,
            compaction: Presence::Present(compaction),
        })
    }))
}

/// Registers custom compaction for this explicitly loaded extension.
///
/// # Errors
/// The returned future returns the host's rejection when it refuses the
/// `session_before_compact` handler registration, so the extension adds no custom summary.
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
