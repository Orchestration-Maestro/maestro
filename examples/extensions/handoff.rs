//! Generates an editable prompt from the current branch for a new, parent-linked session.
// maestro-component: MAESTRO_HANDOFF_COMPONENT
#![cfg(target_arch = "wasm32")]
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use maestro_extensions_wasm::{
    AgentMessage, AssistantContent, BorderedLoader, CommandOptions, Context, CustomDone,
    CustomFactoryOutcome, EventListener, ExtensionAPI, ExtensionCommandContext, ExtensionComponent,
    ExtensionFuture, ExtensionResult, Message, Model, NewSessionCommandData,
    NewSessionCommandOptions, NotificationType, ProviderStreamOptions, ResolvedRequestAuth,
    SessionEntry, StopReason, StreamOptions, TextContent, UserBlock, UserContent, UserMessage,
    complete, convert_to_llm, serialize_conversation, utils,
};
use std::{cell::RefCell, future::poll_fn, rc::Rc, task::Waker, time::SystemTime};

/// Instructions for a focused, self-contained transfer rather than a general summary.
const SYSTEM_PROMPT: &str = r#"You are a context transfer assistant. Given a conversation history and the user's goal for a new thread, generate a focused prompt that:

1. Summarizes relevant context from the conversation (decisions made, approaches taken, key findings)
2. Lists any relevant files that were discussed or modified
3. Clearly states the next task based on the user's goal
4. Is self-contained - the new thread should be able to proceed without the old conversation

Format your response as a prompt the user can send to start the new thread. Be concise but include all necessary context. Do not include any preamble like "Here's the prompt" - just output the prompt itself.

Example output format:
## Context
We've been working on X. Key decisions:
- Decision 1
- Decision 2

Files involved:
- path/to/file1.ts
- path/to/file2.ts

## Task
[Clear description of what to do next based on user's goal]"#;
/// The private event channel handing generation to the host's independent executor.
const CHANNEL: &str = "maestro.example.handoff";

/// One cancellable generation and the host tail waiting for it.
#[derive(Default)]
struct Generation {
    /// Dropping this future releases the outstanding request observation.
    running: Option<ExtensionFuture<'static, ()>>,
    /// Wakes the tail when the loader cancels or closes.
    waker: Option<Waker>,
    /// Prevents another instance's emission from claiming this job twice.
    claimed: bool,
}
/// The factory owns only one active request.
type Current = Rc<RefCell<Option<Rc<RefCell<Generation>>>>>;

/// Releases the pending work and settles its independently waiting tail.
fn cancel(current: &Current) {
    if let Some(job) = current.borrow_mut().take() {
        let mut job = job.borrow_mut();
        job.running = None;
        if let Some(waker) = job.waker.take() {
            waker.wake();
        }
    }
}

/// Polls the one owned job, without inventing another executor.
fn listener(current: Current) -> EventListener {
    Rc::new(move |_, _| {
        let Some(current) = current.borrow().clone() else {
            return Ok(None);
        };
        if std::mem::replace(&mut current.borrow_mut().claimed, true) {
            return Ok(None);
        }
        Ok(Some(Box::pin(poll_fn(move |cx| {
            let mut running = {
                let mut current = current.borrow_mut();
                current.waker = Some(cx.waker().clone());
                current.running.take()
            };
            // Completion can synchronously dispose the loader and cancel this job.
            let result = running
                .as_mut()
                .map_or(std::task::Poll::Ready(Ok(())), |running| {
                    running.as_mut().poll(cx)
                });
            let mut current = current.borrow_mut();
            if result.is_ready() {
                current.waker = None;
            } else if current.waker.is_some() {
                current.running = running;
            }
            result
        }))))
    })
}

/// Selects messages and compaction summaries around the latest compaction.
fn handoff_messages(branch: &[SessionEntry]) -> Vec<AgentMessage> {
    let mut selected = Vec::new();
    let mut start = 0;
    if let Some(index) = branch
        .iter()
        .rposition(|entry| matches!(entry, SessionEntry::Compaction(_)))
        && let SessionEntry::Compaction(compaction) = &branch[index]
    {
        selected.extend(entry_message(&branch[index]));
        if let Some(first) = branch[..index]
            .iter()
            .position(|entry| entry_id(entry) == compaction.first_kept_entry_id)
        {
            selected.extend(branch[first..index].iter().filter_map(entry_message));
        }
        start = index + 1;
    }
    selected.extend(branch[start..].iter().filter_map(entry_message));
    selected
}

/// Reads entry identity without restricting which kind can mark the kept prefix.
fn entry_id(entry: &SessionEntry) -> &str {
    match entry {
        SessionEntry::Message(entry) => &entry.base.id,
        SessionEntry::ThinkingLevelChange(entry) => &entry.base.id,
        SessionEntry::ModelChange(entry) => &entry.base.id,
        SessionEntry::Compaction(entry) => &entry.base.id,
        SessionEntry::BranchSummary(entry) => &entry.base.id,
        SessionEntry::Custom(entry) => &entry.base.id,
        SessionEntry::CustomMessage(entry) => &entry.base.id,
        SessionEntry::Label(entry) => &entry.base.id,
        SessionEntry::SessionInfo(entry) => &entry.base.id,
    }
}

/// Ignores settings, labels, custom entries and branch summaries.
fn entry_message(entry: &SessionEntry) -> Option<AgentMessage> {
    match entry {
        SessionEntry::Message(message) => Some(message.message.clone()),
        SessionEntry::Compaction(compaction) => Some(AgentMessage::CompactionSummary(
            maestro_extensions_wasm::CompactionSummaryMessage {
                summary: compaction.summary.clone(),
                tokens_before: compaction.tokens_before,
                // Transcript serialization does not carry the summary's timestamp.
                timestamp: 0.0,
            },
        )),
        _ => None,
    }
}

/// Resolves configured credentials and collects only response text, in content order.
async fn generate(
    ctx: ExtensionCommandContext,
    model: Model,
    text: String,
    signal: maestro_extensions_wasm::AbortSignal,
) -> ExtensionResult<Option<String>> {
    let auth = ctx
        .model_registry()?
        .get_api_key_and_headers(model.clone())
        .await?;
    let (api_key, headers, base_url) = match auth {
        ResolvedRequestAuth::Ok {
            api_key: Some(key),
            headers,
            base_url,
        } if !key.is_empty() => (key, headers, base_url),
        ResolvedRequestAuth::Ok { .. } => return Err(format!("No API key for {}", model.provider)),
        ResolvedRequestAuth::Error { error } => return Err(error),
    };
    let response = complete(
        model,
        Context {
            system_prompt: Some(SYSTEM_PROMPT.into()),
            messages: vec![Message::User(UserMessage {
                content: UserContent::Blocks(vec![UserBlock::Text(TextContent {
                    text,
                    text_signature: None,
                })]),
                timestamp: SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map_err(|error| error.to_string())?
                    .as_secs_f64()
                    * 1000.0,
            })],
            tools: None,
        },
        Some(ProviderStreamOptions {
            common: StreamOptions {
                api_key: Some(api_key),
                base_url,
                headers,
                signal: Some(signal),
                ..Default::default()
            },
            ..Default::default()
        }),
    )
    .await?;
    let response = response.read().map_err(|error| error.to_string())?;
    if response.stop_reason == StopReason::Aborted {
        return Ok(None);
    }
    Ok(Some(
        response
            .content
            .iter()
            .filter_map(|content| match content {
                AssistantContent::Text(text) => Some(text.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
    ))
}

/// Shows the loader while generation runs as an independently owned event tail.
async fn draft(
    api: ExtensionAPI,
    current: Current,
    ctx: ExtensionCommandContext,
    model: Model,
    text: String,
) -> ExtensionResult<Option<String>> {
    let ui = ctx.ui()?;
    let slot = current.clone();
    let result = ui
        .custom(
            Rc::new(move |tui, theme, _, done: CustomDone<Option<String>>, _| {
                cancel(&slot);
                let loader =
                    BorderedLoader::new(&tui, &theme, "Generating handoff prompt...", None)?;
                let aborted = done.clone();
                let cancelled = slot.clone();
                loader.set_on_abort(Some(Rc::new(move |_| {
                    cancel(&cancelled);
                    aborted(None)
                })))?;
                let (ctx, model, text, signal) =
                    (ctx.clone(), model.clone(), text.clone(), loader.signal()?);
                *slot.borrow_mut() = Some(Rc::new(RefCell::new(Generation {
                    running: Some(Box::pin(async move {
                        let result =
                            generate(ctx, model, text, signal)
                                .await
                                .unwrap_or_else(|error| {
                                    eprintln!("Handoff generation failed: {error}");
                                    None
                                });
                        done(result)
                    })),
                    ..Generation::default()
                })));
                let api = api.clone();
                let disposed = loader.clone();
                let disposed_slot = slot.clone();
                Ok(CustomFactoryOutcome::Async(Box::pin(async move {
                    api.events().emit(CHANNEL, serde_json::Value::Null).await?;
                    Ok(ExtensionComponent {
                        component: (&loader).into(),
                        dispose: Some(Rc::new(move |_| {
                            cancel(&disposed_slot);
                            disposed.dispose()
                        })),
                        set_expanded: None,
                        width: None,
                    })
                })))
            }),
            None,
        )
        .await;
    cancel(&current);
    Ok(result?.flatten())
}

/// Validates the interactive command before touching history or requesting a model response.
async fn handoff(
    api: ExtensionAPI,
    current: Current,
    args: String,
    ctx: ExtensionCommandContext,
) -> ExtensionResult<()> {
    if !ctx.has_ui()? {
        return ctx.ui()?.notify(
            "handoff requires interactive mode",
            Some(NotificationType::Error),
        );
    }
    let Some(model) = ctx.model()? else {
        return ctx
            .ui()?
            .notify("No model selected", Some(NotificationType::Error));
    };
    let mut whitespace_error = None;
    let goal = args.trim_matches(|c: char| {
        utils::is_whitespace_char(&api, &c.to_string()).unwrap_or_else(|error| {
            whitespace_error = Some(error);
            false
        })
    });
    if let Some(error) = whitespace_error {
        return Err(error);
    }
    if goal.is_empty() {
        return ctx.ui()?.notify(
            "Usage: /handoff <goal for new thread>",
            Some(NotificationType::Error),
        );
    }
    let reader = ctx.session_manager()?;
    let messages = handoff_messages(&reader.get_branch(None)?);
    if messages.is_empty() {
        return ctx
            .ui()?
            .notify("No conversation to hand off", Some(NotificationType::Error));
    }
    let conversation = serialize_conversation(&api, &convert_to_llm(&api, &messages)?)?;
    let parent_session = reader.get_session_file()?;
    let text = format!(
        "## Conversation History\n\n{conversation}\n\n## User's Goal for New Thread\n\n{goal}"
    );
    let Some(result) = draft(api, current, ctx.clone(), model, text).await? else {
        return ctx.ui()?.notify("Cancelled", Some(NotificationType::Info));
    };
    review(ctx, result, parent_session).await
}

/// Reviews the prompt and uses only the replacement context after the session changes.
async fn review(
    ctx: ExtensionCommandContext,
    result: String,
    parent_session: Option<String>,
) -> ExtensionResult<()> {
    let ui = ctx.ui()?;
    let Some(edited) = ui
        .editor("Edit handoff prompt".into(), Some(result))
        .await?
    else {
        return ctx.ui()?.notify("Cancelled", Some(NotificationType::Info));
    };
    let outcome = ctx
        .new_session(Some(NewSessionCommandOptions {
            data: NewSessionCommandData { parent_session },
            setup: None,
            with_session: Some(Box::new(move |replacement| {
                Box::pin(async move {
                    replacement.ui()?.set_editor_text(&edited)?;
                    replacement.ui()?.notify(
                        "Handoff ready. Submit when ready.",
                        Some(NotificationType::Info),
                    )
                })
            })),
        }))
        .await?;
    if outcome.cancelled {
        ctx.ui()?
            .notify("New session cancelled", Some(NotificationType::Info))?;
    }
    Ok(())
}

/// Registers the opt-in command and the single listener that owns generation work.
///
/// # Errors
/// Returns host registration failures unchanged.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let current = Current::default();
        let subscription = api.events().on(CHANNEL, listener(current.clone()))?;
        let owner = api.clone();
        let shutdown = current.clone();
        api.register_command(
            "handoff",
            CommandOptions {
                description: Some("Transfer context to a new focused session".into()),
                get_argument_completions: None,
                handler: Rc::new(move |args, ctx| {
                    Box::pin(handoff(owner.clone(), current.clone(), args, ctx))
                }),
            },
        )?;
        api.on(
            "session_shutdown",
            Rc::new(move |_, _| {
                cancel(&shutdown);
                let result = subscription.unsubscribe();
                Box::pin(async move {
                    result?;
                    Ok(None)
                })
            }),
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
