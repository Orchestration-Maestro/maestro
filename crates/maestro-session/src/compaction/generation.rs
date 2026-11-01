//! Eager summary generation through the registered completion owner.
use super::{prompts, utils};
use maestro_models::{
    AssistantContent, BoxFuture, Context, DiagnosticErrorInfo, Message, Model, SimpleStreamOptions,
    StopReason, StreamOptions, TextContent, UserBlock, UserContent, UserMessage,
};
use std::fmt::Write as _;
/// Caller-supplied controls for history and split-turn summary requests.
pub struct SummaryOptions {
    /// Explicit authentication, including an empty key.
    pub api_key: String,
    /// Headers in supplied order.
    pub headers: Option<indexmap::IndexMap<String, String>>,
    /// Shared request cancellation.
    pub signal: Option<maestro_models::Cancellation>,
    /// Additional focus for history summaries only.
    pub custom_instructions: Option<String>,
    /// Explicit reasoning selection; absent and off omit reasoning.
    pub thinking_level: Option<maestro_agent::ThinkingLevel>,
}
/// Summary text and the provider usage of the request that produced it.
#[derive(Clone, Debug, PartialEq)]
pub struct GeneratedSummary {
    /// The summary text.
    pub text: String,
    /// Usage the provider reported for the request.
    pub usage: maestro_models::Usage,
}
/// Start a history summary now, returning an independently owned observation.
///
/// Projection errors precede invocation; adapter setup errors retain their diagnostics.
/// Error responses fail with the summary error prefix; other outcomes retain available text.
#[must_use]
pub fn generate_summary(
    current_messages: &[serde_json::Value],
    model: &Model,
    reserve_tokens: f64,
    previous_summary: Option<&str>,
    options: &SummaryOptions,
) -> BoxFuture<Result<String, DiagnosticErrorInfo>> {
    let summary = generate_summary_with_usage(
        current_messages,
        model,
        reserve_tokens,
        previous_summary,
        options,
    );
    Box::pin(async move { Ok(summary.await?.text) })
}
/// Start a history summary like [`generate_summary`], keeping the provider usage.
///
/// The usage is the request's own, whatever its outcome: an aborted request reports what it
/// consumed with the partial text.
#[must_use]
pub fn generate_summary_with_usage(
    current_messages: &[serde_json::Value],
    model: &Model,
    reserve_tokens: f64,
    previous_summary: Option<&str>,
    options: &SummaryOptions,
) -> BoxFuture<Result<GeneratedSummary, DiagnosticErrorInfo>> {
    let previous = previous_summary.filter(|text| !text.is_empty());
    let base = if previous.is_some() {
        prompts::UPDATE_SUMMARIZATION_PROMPT
    } else {
        prompts::SUMMARIZATION_PROMPT
    };
    let prompt = utils::serialize_raw_conversation(current_messages).map(|conversation| {
        let mut text = format!("<conversation>\n{conversation}\n</conversation>\n\n");
        if let Some(previous) = previous {
            let _ = write!(
                text,
                "<previous-summary>\n{previous}\n</previous-summary>\n\n"
            );
        }
        text.push_str(base);
        if let Some(focus) = options
            .custom_instructions
            .as_deref()
            .filter(|text| !text.is_empty())
        {
            let _ = write!(text, "\n\nAdditional focus: {focus}");
        }
        text
    });
    request(
        prompt,
        model,
        (0.8 * reserve_tokens).floor(),
        options,
        "Summarization failed",
    )
}
/// Invoke the completion before returning its independently owned text observation.
fn request(
    prompt: Result<String, serde_json::Error>,
    model: &Model,
    max_tokens: f64,
    options: &SummaryOptions,
    error_prefix: &'static str,
) -> BoxFuture<Result<GeneratedSummary, DiagnosticErrorInfo>> {
    let prompt = match prompt {
        Ok(prompt) => prompt,
        Err(error) => {
            return Box::pin(async move { Err(DiagnosticErrorInfo::error(error.to_string())) });
        }
    };
    let completion = maestro_models::complete_simple(
        model.clone(),
        Context {
            system_prompt: Some(utils::SUMMARIZATION_SYSTEM_PROMPT.into()),
            messages: vec![Message::User(UserMessage {
                content: UserContent::Blocks(vec![UserBlock::Text(TextContent::new(prompt))]),
                timestamp: maestro_models::timestamp_now(),
            })],
            tools: None,
        },
        Some(SimpleStreamOptions {
            common: StreamOptions {
                max_tokens: Some(max_tokens),
                api_key: Some(options.api_key.clone()),
                headers: options.headers.clone(),
                signal: options.signal.clone(),
                ..Default::default()
            },
            reasoning: reasoning(model, options),
            ..Default::default()
        }),
    );
    Box::pin(async move {
        let response = completion.await?;
        let response = response
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if response.stop_reason == StopReason::Error {
            let message = response
                .error_message
                .as_deref()
                .filter(|text| !text.is_empty())
                .unwrap_or("Unknown error");
            return Err(DiagnosticErrorInfo::error(format!(
                "{error_prefix}: {message}"
            )));
        }
        Ok(GeneratedSummary {
            text: response
                .content
                .iter()
                .filter_map(|block| match block {
                    AssistantContent::Text(text) => Some(text.text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n"),
            usage: response.usage.clone(),
        })
    })
}

/// Start prepared compaction without writing or modifying history.
///
/// Both split requests start eagerly when history and prefix are nonempty.
/// Empty history with a nonempty prefix starts only the prefix request; an empty
/// prefix starts only the history request. A first failure does not cancel its sibling.
/// Generation failures precede the empty retained-ID diagnostic.
#[must_use]
pub fn compact(
    preparation: &super::CompactionPreparation,
    model: &Model,
    options: &SummaryOptions,
) -> BoxFuture<Result<super::CompactionResult<super::CompactionDetails>, DiagnosticErrorInfo>> {
    use maestro_request::presence::Presence;
    let previous = preparation.previous_summary.present().map(String::as_str);
    let reserve = preparation.settings.reserve_tokens;
    let history = if preparation.is_split_turn
        && !preparation.turn_prefix_messages.is_empty()
        && preparation.messages_to_summarize.is_empty()
    {
        Box::pin(async { Ok("No prior history.".to_owned()) }) as BoxFuture<_>
    } else {
        generate_summary(
            &preparation.messages_to_summarize,
            model,
            reserve,
            previous,
            options,
        )
    };
    let prefix =
        (preparation.is_split_turn && !preparation.turn_prefix_messages.is_empty()).then(|| {
            generate_turn_prefix_summary(&preparation.turn_prefix_messages, model, reserve, options)
        });
    let details = utils::compute_file_lists(&preparation.file_ops);
    let id = preparation.first_kept_entry_id.clone();
    let tokens_before = preparation.tokens_before;
    Box::pin(async move {
        let mut summary = if let Some(prefix) = prefix {
            let (history, prefix) = futures_util::future::try_join(history, prefix).await?;
            format!("{history}\n\n---\n\n**Turn Context (split turn):**\n\n{prefix}")
        } else {
            history.await?
        };
        summary.push_str(&utils::format_file_operations(
            &details.read_files,
            &details.modified_files,
        ));
        if id.is_empty() {
            return Err(DiagnosticErrorInfo::error(
                "First kept entry has no UUID - session may need migration",
            ));
        }
        Ok(super::CompactionResult {
            summary,
            first_kept_entry_id: id,
            tokens_before,
            details: Presence::Present(details),
        })
    })
}

/// Gate the explicit application selection on model reasoning capability.
fn reasoning(model: &Model, options: &SummaryOptions) -> Option<maestro_models::ThinkingLevel> {
    options
        .thinking_level
        .as_ref()
        .filter(|_| model.reasoning)
        .and_then(|level| match level {
            maestro_agent::ThinkingLevel::Off => None,
            maestro_agent::ThinkingLevel::Minimal => Some(maestro_models::ThinkingLevel::Minimal),
            maestro_agent::ThinkingLevel::Low => Some(maestro_models::ThinkingLevel::Low),
            maestro_agent::ThinkingLevel::Medium => Some(maestro_models::ThinkingLevel::Medium),
            maestro_agent::ThinkingLevel::High => Some(maestro_models::ThinkingLevel::High),
            maestro_agent::ThinkingLevel::Xhigh => Some(maestro_models::ThinkingLevel::Xhigh),
        })
}

/// Start the prefix request without history focus or a previous summary.
fn generate_turn_prefix_summary(
    messages: &[serde_json::Value],
    model: &Model,
    reserve: f64,
    options: &SummaryOptions,
) -> BoxFuture<Result<String, DiagnosticErrorInfo>> {
    let prompt = utils::serialize_raw_conversation(messages).map(|conversation| {
        format!(
            "<conversation>\n{conversation}\n</conversation>\n\n{}",
            prompts::TURN_PREFIX_SUMMARIZATION_PROMPT
        )
    });
    let summary = request(
        prompt,
        model,
        (0.5 * reserve).floor(),
        options,
        "Turn prefix summarization failed",
    );
    Box::pin(async move { Ok(summary.await?.text) })
}
