//! The author extension shared by the example component and the tests.
//!
//! It is written only against the author facade, so the same code runs as a component and
//! under the controlled adapter. Every registered closure captures a [`Released`] guard that
//! reports through the host when the closure is dropped.
use std::cell::RefCell;
use std::rc::Rc;

use maestro_extensions_wasm::{
    AbortSignal, AgentToolResult, BashToolCallEvent, CommandOptions, CompactionResult,
    ContentBlock, ExtensionAPI, ExtensionEvent, ExtensionEventResult, ExtensionFuture,
    ExtensionHandler, InputEventResult, InputTransform, NewSessionCommandData,
    NewSessionCommandOptions, PrepareArguments, SessionBeforeCompactResult, SessionEvent,
    TextContent, ToolCallEvent, ToolDefinition, ToolExecute, ToolMetadata, UserContent,
};
use serde_json::json;

/// Reports its label to the host when the closure that captured it is dropped.
struct Released {
    /// Extension handle used to report the release.
    api: ExtensionAPI,
    /// Name reported on release.
    label: &'static str,
}

impl Released {
    /// A guard that reports `label` through `api`.
    fn new(api: &ExtensionAPI, label: &'static str) -> Self {
        Self {
            api: api.clone(),
            label,
        }
    }
}

impl Drop for Released {
    fn drop(&mut self) {
        let _ = self.api.append_entry("released", Some(json!(self.label)));
    }
}

/// The extension factory both adapters run.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        register_input(&api)?;
        register_tool_call(&api)?;
        register_compaction(&api)?;
        register_tool(&api)?;
        register_command(&api)?;
        register_rejected(&api)?;
        register_reentrant(&api)
    })
}

/// Registers the input handler: it upper-cases every input.
fn register_input(api: &ExtensionAPI) -> Result<(), String> {
    let released = Released::new(api, "input-handler");
    let log = api.clone();
    api.on(
        "input",
        Rc::new(move |event, ctx| {
            let _ = &released;
            let log = log.clone();
            Box::pin(async move {
                let ExtensionEvent::Input(input) = event else {
                    return Ok(None);
                };
                log.append_entry(
                    "input",
                    Some(json!({ "text": input.text, "cwd": ctx.cwd()? })),
                )?;
                Ok(Some(ExtensionEventResult::Input(
                    InputEventResult::Transform(InputTransform {
                        text: input.text.to_uppercase(),
                        images: input.images.clone(),
                    }),
                )))
            })
        }),
    )
}

/// Registers the tool call handler: it edits the command of a bash call, then fails.
fn register_tool_call(api: &ExtensionAPI) -> Result<(), String> {
    let released = Released::new(api, "tool-call-handler");
    api.on(
        "tool_call",
        Rc::new(move |event, _ctx| {
            let _ = &released;
            Box::pin(async move {
                if let ExtensionEvent::ToolCall(ToolCallEvent::Bash(BashToolCallEvent {
                    input,
                    ..
                })) = event
                {
                    input.command.push_str(" -la");
                    return Err("tool call blocked".to_owned());
                }
                Ok(None)
            })
        }),
    )
}

/// Registers the compaction handler: it answers with the summary of its previous signal.
fn register_compaction(api: &ExtensionAPI) -> Result<(), String> {
    let released = Released::new(api, "compaction-handler");
    let remembered: Rc<RefCell<Option<AbortSignal>>> = Rc::default();
    api.on(
        "session_before_compact",
        Rc::new(move |event, _ctx| {
            let _ = &released;
            let remembered = Rc::clone(&remembered);
            Box::pin(async move {
                let ExtensionEvent::Session(SessionEvent::BeforeCompact(compact)) = event else {
                    return Ok(None);
                };
                let previous = remembered.borrow().as_ref().map(AbortSignal::aborted);
                remembered.replace(Some(compact.signal.clone()));
                Ok(Some(ExtensionEventResult::SessionBeforeCompact(
                    SessionBeforeCompactResult {
                        cancel: Some(compact.signal.aborted()),
                        compaction: Some(CompactionResult {
                            summary: format!("previous_aborted:{previous:?}"),
                            first_kept_entry_id: compact.preparation.first_kept_entry_id.clone(),
                            tokens_before: compact.preparation.tokens_before,
                            details: None,
                        }),
                    },
                )))
            })
        }),
    )
}

/// Registers a handler the host rejects, then reports the rejection.
fn register_rejected(api: &ExtensionAPI) -> Result<(), String> {
    let released = Released::new(api, "rejected-handler");
    let handler: ExtensionHandler = Rc::new(move |_event, _ctx| {
        let _ = &released;
        Box::pin(async { Ok(None) })
    });
    if let Err(message) = api.on("rejected", handler) {
        api.append_entry("rejection", Some(json!(message)))?;
    }
    Ok(())
}

/// Registers a handler whose captured guard registers another handler when it is dropped.
fn register_reentrant(api: &ExtensionAPI) -> Result<(), String> {
    let guard = Reentrant { api: api.clone() };
    api.on(
        "reentrant",
        Rc::new(move |_event, _ctx| {
            let _ = &guard;
            Box::pin(async { Ok(None) })
        }),
    )
}

/// A guard that registers a handler from its destructor, while its own entry is being released.
struct Reentrant {
    /// Extension handle used to report and register.
    api: ExtensionAPI,
}

impl Drop for Reentrant {
    fn drop(&mut self) {
        let _ = self
            .api
            .append_entry("released", Some(json!("reentrant-guard")));
        let _ = self
            .api
            .on("late", Rc::new(|_event, _ctx| Box::pin(async { Ok(None) })));
    }
}

/// Text content with a provider signature.
fn text(text: &str, signature: Option<&str>) -> ContentBlock {
    ContentBlock::Text(TextContent {
        text: text.to_owned(),
        text_signature: signature.map(str::to_owned),
    })
}

/// Registers the echo tool: it marks its arguments as prepared, reports progress and
/// answers with the call it saw and whether its previous signal was cancelled.
fn register_tool(api: &ExtensionAPI) -> Result<(), String> {
    let prepare_released = Released::new(api, "tool-prepare");
    let prepare: PrepareArguments = Rc::new(move |mut params, emitter| {
        let _ = &prepare_released;
        params["prepared"] = json!(true);
        emitter.emit("tool:prepared", params.clone())?;
        Ok(params)
    });
    let released = Released::new(api, "tool-execute");
    let remembered: Rc<RefCell<Option<AbortSignal>>> = Rc::default();
    let execute: ToolExecute = Rc::new(move |call_id, params, signal, update, _ctx| {
        let _ = &released;
        let previous = remembered.borrow().as_ref().map(AbortSignal::aborted);
        remembered.replace(signal);
        Box::pin(async move {
            if let Some(update) = update {
                update(AgentToolResult {
                    content: vec![text("working", None)],
                    details: None,
                    terminate: None,
                })?;
            }
            let said = params["text"].as_str().unwrap_or_default();
            Ok(AgentToolResult {
                content: vec![text(&format!("echo {said}"), Some("sig-é"))],
                details: Some(json!({ "call": call_id, "previous_aborted": previous }).to_string()),
                terminate: Some(true),
            })
        })
    });
    api.register_tool(ToolDefinition {
        metadata: ToolMetadata {
            name: "echo".to_owned(),
            label: "Echo".to_owned(),
            description: "Echoes its text".to_owned(),
            prompt_snippet: None,
            prompt_guidelines: None,
            parameters: json!({ "type": "object" }).to_string(),
            render_shell: None,
            execution_mode: None,
        },
        prepare_arguments: Some(prepare),
        execute,
    })
}

/// Registers the command that starts a new session and greets from its continuation.
fn register_command(api: &ExtensionAPI) -> Result<(), String> {
    let released = Released::new(api, "command-handler");
    let log = api.clone();
    api.register_command(
        "replace",
        CommandOptions {
            description: Some("Replaces the session".to_owned()),
            get_argument_completions: None,
            handler: Rc::new(move |args, ctx| {
                let _ = &released;
                let log = log.clone();
                Box::pin(async move {
                    let before = ctx.cwd()?;
                    log.append_entry("command", Some(json!({ "args": args, "cwd": before })))?;
                    let outcome = ctx.new_session(Some(replacement(&log))).await?;
                    if let Err(stale) = ctx.cwd() {
                        log.append_entry("stale", Some(json!(stale)))?;
                    }
                    log.append_entry(
                        "after",
                        Some(json!({ "before": before, "cancelled": outcome.cancelled })),
                    )
                })
            }),
        },
    )
}

/// Options whose continuation runs against the replacement session.
fn replacement(log: &ExtensionAPI) -> NewSessionCommandOptions {
    let released = Released::new(log, "continuation");
    let log = log.clone();
    NewSessionCommandOptions {
        data: NewSessionCommandData {
            parent_session: Some("parent".to_owned()),
        },
        setup: None,
        with_session: Some(Box::new(move |replaced| {
            Box::pin(async move {
                let _ = &released;
                log.append_entry("continuation", Some(json!({ "cwd": replaced.cwd()? })))?;
                replaced
                    .send_user_message(
                        UserContent::Text("hello from continuation".to_owned()),
                        None,
                    )
                    .await
            })
        })),
    }
}
