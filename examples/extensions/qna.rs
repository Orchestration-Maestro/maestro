//! Extracts questions from the last complete assistant reply into the editor.
// Tests receive this component's path in MAESTRO_QNA_COMPONENT.
#![cfg(target_arch = "wasm32")]
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use maestro_extensions_wasm::{
    AgentMessage, AssistantContent, BorderedLoader, CommandOptions, Context, CustomDone,
    CustomFactory, CustomFactoryOutcome, EventListener, ExtensionAPI, ExtensionCommandContext,
    ExtensionComponent, ExtensionFuture, ExtensionResult, Message, NotificationType,
    ProviderStreamOptions, ResolvedRequestAuth, SessionEntry, StopReason, StreamOptions,
    TextContent, UserBlock, UserContent, UserMessage, complete,
};
use std::{
    cell::RefCell,
    rc::Rc,
    time::{SystemTime, UNIX_EPOCH},
};

/// Instructions and output format supplied to the selected model.
const SYSTEM_PROMPT: &str = "You are a question extractor. Given text from a conversation, extract any questions that need answering and format them for the user to fill in.

Output format:
- List each question on its own line, prefixed with \"Q: \"
- After each question, add a blank line for the answer prefixed with \"A: \"
- If no questions are found, output \"No questions found in the last message.\"

Example output:
Q: What is your preferred database?
A:\x20

Q: Should we use TypeScript or JavaScript?
A:\x20

Keep questions in the order they appeared. Be concise.";
/// The channel that hands extraction to the host's independent event-tail executor.
const CHANNEL: &str = "maestro.example.qna";

#[path = "support/tail.rs"]
mod tail;
use tail::Tail;

/// Work waiting to be handed to the listener once.
type Pending = Rc<RefCell<Option<Tail>>>;

/// Hands the loader's work to the shared cancellable tail owner.
fn listener(pending: Pending) -> EventListener {
    Rc::new(move |payload, emitter| {
        pending
            .borrow_mut()
            .take()
            .map_or(Ok(None), |tail| (tail.listener())(payload, emitter))
    })
}

/// Joins only text blocks, preserving their order, whitespace and empty strings.
fn text_output(content: &[AssistantContent]) -> String {
    content
        .iter()
        .filter_map(|part| match part {
            AssistantContent::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Reads only the active branch and refuses an incomplete newest assistant reply.
fn last_assistant(ctx: &ExtensionCommandContext) -> ExtensionResult<String> {
    for entry in ctx.session_manager()?.get_branch(None)?.iter().rev() {
        if let SessionEntry::Message(entry) = entry
            && let AgentMessage::Message(message) = &entry.message
            && let Message::Assistant(message) = message.as_ref()
        {
            if message.stop_reason != StopReason::Stop {
                let stop = serde_json::to_value(&message.stop_reason)
                    .map_err(|error| error.to_string())?;
                return Err(format!(
                    "Last assistant message incomplete ({})",
                    stop.as_str().unwrap_or_default()
                ));
            }
            if message
                .content
                .iter()
                .any(|part| matches!(part, AssistantContent::Text(_)))
            {
                let text = text_output(&message.content);
                return (!text.is_empty())
                    .then_some(text)
                    .ok_or_else(|| "No assistant messages found".into());
            }
        }
    }
    Err("No assistant messages found".into())
}

/// Resolves authentication for one captured model and observes its completion.
async fn extract(
    ctx: ExtensionCommandContext,
    text: String,
    loader: BorderedLoader,
) -> ExtensionResult<Option<String>> {
    let model = ctx.model()?.ok_or_else(|| "No model selected".to_owned())?;
    // Keep the request model paired with its credentials across authentication.
    let auth = ctx
        .model_registry()?
        .get_api_key_and_headers(model.clone())
        .await?;
    let ResolvedRequestAuth::Ok {
        api_key: Some(api_key),
        headers,
    } = auth
    else {
        return Ok(None);
    };
    if api_key.is_empty() {
        return Ok(None);
    }
    let timestamp = (SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_secs_f64()
        * 1000.0)
        .floor();
    let response = complete(
        model,
        Context {
            system_prompt: Some(SYSTEM_PROMPT.into()),
            messages: vec![Message::User(UserMessage {
                content: UserContent::Blocks(vec![UserBlock::Text(TextContent::new(text))]),
                timestamp,
            })],
            tools: None,
        },
        Some(ProviderStreamOptions {
            common: StreamOptions {
                api_key: Some(api_key),
                headers,
                signal: Some(loader.signal()?),
                ..Default::default()
            },
            ..Default::default()
        }),
    )
    .await?;
    let response = response.read().map_err(|error| error.to_string())?;
    Ok((!matches!(
        response.stop_reason,
        StopReason::Aborted | StopReason::Error
    ))
    .then(|| text_output(&response.content)))
}

/// Builds the cancellable loader and starts its independently owned extraction.
fn loader_factory(
    pending: Pending,
    ctx: ExtensionCommandContext,
    text: String,
) -> CustomFactory<Option<String>> {
    Rc::new(
        move |tui, theme, _, done: CustomDone<Option<String>>, emitter| {
            let current = Tail::default();
            let model = ctx.model()?.ok_or_else(|| "No model selected".to_owned())?;
            let loader = BorderedLoader::new(
                &tui,
                &theme,
                &format!("Extracting questions using {}...", model.id),
                None,
            )?;
            let aborted = done.clone();
            loader.set_on_abort(Some(Rc::new(move |_| aborted(None))))?;
            let (context, text, extracting) = (ctx.clone(), text.clone(), loader.clone());
            current.start(Box::pin(async move {
                let result = extract(context, text, extracting).await.unwrap_or_default();
                done(result)
            }));
            pending.replace(Some(current.clone()));
            if let Err(error) = emitter.emit(CHANNEL, serde_json::Value::Null) {
                pending.borrow_mut().take();
                current.stop();
                loader.dispose()?;
                return Err(error);
            }
            Ok(CustomFactoryOutcome::Ready(ExtensionComponent {
                component: (&loader).into(),
                dispose: Some(Rc::new(move |_| {
                    current.stop();
                    loader.dispose()
                })),
                set_expanded: None,
                width: None,
            }))
        },
    )
}

/// Validates the request, then puts the completion's exact text into the editable input.
async fn command(pending: Pending, ctx: ExtensionCommandContext) -> ExtensionResult<()> {
    if !ctx.has_ui()? {
        return ctx.ui()?.notify(
            "qna requires interactive mode",
            Some(NotificationType::Error),
        );
    }
    if ctx.model()?.is_none() {
        return ctx
            .ui()?
            .notify("No model selected", Some(NotificationType::Error));
    }
    let text = match last_assistant(&ctx) {
        Ok(text) => text,
        Err(error) => return ctx.ui()?.notify(&error, Some(NotificationType::Error)),
    };
    let result = ctx
        .ui()?
        .custom(loader_factory(pending, ctx.clone(), text), None)
        .await?
        .flatten();
    let Some(result) = result else {
        return ctx.ui()?.notify("Cancelled", Some(NotificationType::Info));
    };
    ctx.ui()?.set_editor_text(&result)?;
    ctx.ui()?.notify(
        "Questions loaded. Edit and submit when ready.",
        Some(NotificationType::Info),
    )
}

/// Registers the interactive question extraction command and its host-owned event tail.
///
/// # Errors
/// Returns registration failures unchanged.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let pending = Pending::default();
        api.events().on(CHANNEL, listener(pending.clone()))?;
        api.register_command(
            "qna",
            CommandOptions {
                description: Some(
                    "Extract questions from last assistant message into editor".into(),
                ),
                get_argument_completions: None,
                handler: Rc::new(move |_, ctx| Box::pin(command(pending.clone(), ctx))),
            },
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
