//! The author extension shared by the example component and the tests.
//!
//! It is written only against the author facade, so the same code runs as a component and
//! under the controlled adapter. Closures with a [`Released`] guard report through the host
//! when that guard is dropped.
use std::cell::RefCell;
use std::rc::Rc;

use maestro_extensions_wasm::{
    AbortSignal, CommandOptions, CompactionResult, ExtensionAPI, ExtensionCommandContext,
    ExtensionContext, ExtensionEvent, ExtensionEventResult, ExtensionFuture, ExtensionHandler,
    ExtensionResult, InputEventResult, InputTransform, NewSessionCommandData,
    NewSessionCommandOptions, Presence, SessionBeforeCompactEvent, SessionBeforeCompactResult,
    SessionEvent, SignalPort,
};
use serde::Deserialize;
use serde_json::{Value, from_value, json};

#[path = "reader_author.rs"]
mod reader_author;
#[path = "tool_author.rs"]
mod tool_author;

/// Reports its label to the host when this guard is dropped.
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
        register_compaction(&api)?;
        register_command(&api)?;
        register_rejected(&api)?;
        register_reentrant(&api)?;
        register_trim(&api)?;
        register_note(&api)?;
        register_probe(&api)
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

/// Registers the compaction handler: it cancels an aborted compaction and answers with the
/// state of the signal it retained from the previous one.
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
                        cancel: Presence::Present(compact.signal.aborted()),
                        compaction: Presence::Present(CompactionResult {
                            summary: format!("previous_aborted:{previous:?}"),
                            first_kept_entry_id: compact.preparation.first_kept_entry_id.clone(),
                            tokens_before: compact.preparation.tokens_before,
                            details: Presence::Missing,
                        }),
                    },
                )))
            })
        }),
    )
}

/// Registers the handler that trims an input in place and fails when nothing is left.
fn register_trim(api: &ExtensionAPI) -> Result<(), String> {
    api.on(
        "trim_input",
        Rc::new(|event, _ctx| {
            Box::pin(async move {
                let ExtensionEvent::Input(input) = event else {
                    return Ok(None);
                };
                input.text = input.text.trim().to_owned();
                if input.text.is_empty() {
                    return Err("nothing is left of the input after trimming".to_owned());
                }
                Ok(None)
            })
        }),
    )
}

/// Registers the handler that notes the tokens in the compaction preparation and fails when
/// the compaction was aborted.
fn register_note(api: &ExtensionAPI) -> Result<(), String> {
    api.on(
        "note_compaction",
        Rc::new(|event, _ctx| {
            Box::pin(async move {
                let ExtensionEvent::Session(SessionEvent::BeforeCompact(compact)) = event else {
                    return Ok(None);
                };
                let tokens = compact.preparation.tokens_before;
                compact.preparation.previous_summary =
                    Presence::Present(format!("noted {tokens} tokens"));
                if compact.signal.aborted() {
                    return Err("compaction aborted after the note".to_owned());
                }
                Ok(None)
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

/// Registers the command that starts a new session and waits for idle from its continuation.
fn register_command(api: &ExtensionAPI) -> Result<(), String> {
    let released = Released::new(api, "command-handler");
    let log = api.clone();
    api.register_command(
        "replace",
        CommandOptions {
            description: Some("Replaces the session".to_owned()),
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
        with_session: Some(Box::new(move |replaced| {
            Box::pin(async move {
                let _ = &released;
                log.append_entry("continuation", Some(json!({ "cwd": replaced.cwd()? })))?;
                replaced.wait_for_idle().await
            })
        })),
    }
}

/// How the probe handler ends, told by the test.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
enum Ending {
    /// Returns no result.
    #[default]
    Silently,
    /// Returns a result of the named family, decoded from the value.
    Returns {
        /// The event whose result contract the value follows.
        family: String,
        /// The result document.
        value: Value,
    },
    /// Fails with this message.
    Fails(String),
}

/// Which resources of an invocation the probe handler keeps.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
struct Retain {
    /// Keep the context.
    context: bool,
    /// Keep the compaction signal.
    signal: bool,
}

/// What the probe handler does, told by the test as JSON in the working directory of its
/// context: it reads the directive from the context it is given.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
struct Directive {
    /// Resources to keep past this invocation.
    retain: Retain,
    /// Report every kept context and signal through an entry.
    report: bool,
    /// Wait for idle on the command context a command captured, before anything else.
    wait: bool,
    /// Edit the event in place.
    mark: bool,
    /// Compatibility member to edit on the selected descriptor.
    compatibility_edit: Option<Value>,
    /// A session example operation to execute.
    session_action: Option<SessionAction>,
    /// Apply the recorded session edit before ending the handler.
    session_edit: Option<String>,

    /// Replace the event with the event this document describes.
    replace_with: Option<Value>,
    /// Bits to assign to a numeric event field.
    number_bits: Option<String>,
    /// Numeric session field selected for a typed assignment.
    number_field: Option<String>,
    /// Number bits to put in a returned compaction.
    result_number_bits: Option<String>,
    /// How the handler ends.
    ending: Ending,
}

/// The session example operation requested by the probe driver.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum SessionAction {
    /// Assemble a user-request summary.
    Summary,
    /// Remove and reinsert a literal read path.
    FileEdit,
}

/// The resources one invocation of the probe handler kept.
struct Kept {
    /// The kept context.
    context: Option<ExtensionContext>,
    /// The kept compaction signal.
    signal: Option<AbortSignal>,
}

/// A signal that is never cancelled.
struct Live;

impl SignalPort for Live {
    fn aborted(&self) -> bool {
        false
    }
}

/// What the probe handler and the command that feeds it share.
#[derive(Clone)]
struct Probe {
    /// Reports through entries.
    api: ExtensionAPI,
    /// The command context the capture command last received.
    captured: Rc<RefCell<Option<ExtensionCommandContext>>>,
    /// Everything invocations kept.
    kept: Rc<RefCell<Vec<Kept>>>,
}

/// Registers the handler that does what each delivery tells it, and the command that captures
/// a command context for it to wait on.
fn register_probe(api: &ExtensionAPI) -> Result<(), String> {
    let probe = Probe {
        api: api.clone(),
        captured: Rc::default(),
        kept: Rc::default(),
    };
    let captured = Rc::clone(&probe.captured);
    let tool_api = api.clone();
    api.on(
        "probe",
        Rc::new(move |event, ctx| {
            let probe = probe.clone();
            Box::pin(async move { probe.run(event, ctx).await })
        }),
    )?;
    api.register_command(
        "capture",
        CommandOptions {
            description: None,
            handler: Rc::new(move |args, ctx| {
                let api = tool_api.clone();
                let captured = Rc::clone(&captured);
                Box::pin(async move {
                    if args == "reader" {
                        return reader_author::register(&api);
                    }
                    if let Some(config) = args.strip_prefix("tools ") {
                        return tool_author::register(&api, config, Rc::clone(&captured));
                    }
                    captured.replace(Some(ctx));
                    Ok(())
                })
            }),
        },
    )
}

impl Probe {
    /// Does what the directive in the working directory of `ctx` says.
    async fn run(
        self,
        event: &mut ExtensionEvent,
        ctx: ExtensionContext,
    ) -> ExtensionResult<Option<ExtensionEventResult>> {
        self.api.append_entry("entered", None)?;
        let directive: Directive = serde_json::from_str(&ctx.cwd()?)
            .map_err(|error| format!("unreadable directive: {error}"))?;
        self.keep(&directive.retain, event, &ctx);
        if matches!(directive.session_action, Some(SessionAction::Summary)) {
            return summarize(event).map(Some);
        }
        if let Some(bits) = directive.number_bits.as_deref() {
            write_number(event, bits, directive.number_field.as_deref())?;
        }
        if directive.wait {
            let command = self.captured.borrow().clone();
            command
                .ok_or("no command context was captured")?
                .wait_for_idle()
                .await?;
        }
        apply_event_edits(event, &directive)?;
        if directive.mark {
            mark(event);
        }
        if let Some(document) = directive.replace_with {
            *event = event_from(document)?;
        }
        if directive.report {
            self.report()?;
        }
        if let Some(bits) = directive.result_number_bits {
            let mut result = match &directive.ending {
                Ending::Returns { family, value } => reply(family, value.clone())?,
                _ => reply(
                    "session_before_compact",
                    json!({"compaction":{"summary":"s","firstKeptEntryId":"e","tokensBefore":0.0}}),
                )?,
            };
            let number = f64::from_bits(u64::from_str_radix(&bits, 16).map_err(|e| e.to_string())?);
            assign_result_number(&mut result, number);
            if let ExtensionEventResult::SessionBeforeCompact(compact) = &mut result
                && let Presence::Present(compaction) = &mut compact.compaction
            {
                compaction.tokens_before =
                    f64::from_bits(u64::from_str_radix(&bits, 16).map_err(|e| e.to_string())?);
            }
            return Ok(Some(result));
        }
        match directive.ending {
            Ending::Silently => Ok(None),
            Ending::Returns { family, value } => reply(&family, value).map(Some),
            Ending::Fails(message) => Err(message),
        }
    }

    /// Keeps what `retain` asks for.
    fn keep(&self, retain: &Retain, event: &ExtensionEvent, ctx: &ExtensionContext) {
        let signal = match event {
            ExtensionEvent::Session(SessionEvent::BeforeCompact(compact)) => {
                Some(compact.signal.clone())
            }
            ExtensionEvent::Session(SessionEvent::BeforeTree(tree)) => Some(tree.signal.clone()),
            _ => None,
        };
        let kept = Kept {
            context: retain.context.then(|| ctx.clone()),
            signal: signal.filter(|_| retain.signal),
        };
        if kept.context.is_some() || kept.signal.is_some() {
            self.kept.borrow_mut().push(kept);
        }
    }

    /// Reports the working directory and cancellation state of everything kept.
    fn report(&self) -> ExtensionResult<()> {
        let kept: Vec<Value> = self
            .kept
            .borrow()
            .iter()
            .map(|kept| {
                json!({
                    "cwd": kept.context.as_ref().map(|ctx| ctx.cwd().unwrap_or_default()),
                    "aborted": kept.signal.as_ref().map(AbortSignal::aborted),
                })
            })
            .collect();
        self.api.append_entry("kept", Some(Value::Array(kept)))
    }
}

/// Assigns a numeric field in the new result families.
fn assign_result_number(result: &mut ExtensionEventResult, number: f64) {
    if let ExtensionEventResult::UserBash(event) = result
        && let Presence::Present(result) = &mut event.result
    {
        result.exit_code = Presence::Present(number);
        return;
    }
    let message = match result {
        ExtensionEventResult::Context(context) => match &mut context.messages {
            Presence::Present(messages) => messages.first_mut(),
            _ => None,
        },
        ExtensionEventResult::MessageEnd(end) => match &mut end.message {
            Presence::Present(message) => Some(message),
            _ => None,
        },
        _ => None,
    };
    if let Some(maestro_extensions_wasm::AgentMessage::Message(message)) = message
        && let maestro_extensions_wasm::Message::User(user) = message.as_mut()
    {
        user.timestamp = number;
    }
}

/// Edits the field of the event that tells the kinds apart.
fn mark(event: &mut ExtensionEvent) {
    let changed = || "changed".to_owned();
    match event {
        ExtensionEvent::ModelSelect(event) => event.model.name = changed(),
        ExtensionEvent::ThinkingLevelSelect(event) => {
            event.level = maestro_extensions_wasm::ThinkingLevel::Off;
        }
        ExtensionEvent::ToolExecutionStart(event) => event.tool_name = changed(),
        ExtensionEvent::ToolExecutionUpdate(event) => event.tool_name = changed(),
        ExtensionEvent::ToolExecutionEnd(event) => event.tool_name = changed(),
        ExtensionEvent::Context(event) => event.messages.reverse(),
        ExtensionEvent::AgentEnd(event) => event.messages.reverse(),
        ExtensionEvent::BeforeAgentStart(event) => event.prompt = changed(),
        ExtensionEvent::AgentStart(_) => {}
        ExtensionEvent::TurnStart(event) => event.turn_index = 99.0,
        ExtensionEvent::TurnEnd(event) => event.turn_index = 99.0,
        ExtensionEvent::MessageStart(event) => mark_message(&mut event.message),
        ExtensionEvent::MessageUpdate(event) => mark_message(&mut event.message),
        ExtensionEvent::MessageEnd(event) => mark_message(&mut event.message),
        ExtensionEvent::ResourcesDiscover(discover) => discover.cwd = changed(),
        ExtensionEvent::Session(SessionEvent::Start(start)) => {
            start.previous_session_file = Presence::Present(changed());
        }
        ExtensionEvent::Session(SessionEvent::BeforeSwitch(switch)) => {
            switch.target_session_file = Presence::Present(changed());
        }
        ExtensionEvent::Session(SessionEvent::BeforeFork(fork)) => fork.entry_id = changed(),
        ExtensionEvent::Session(SessionEvent::BeforeCompact(compact)) => {
            compact.custom_instructions = Presence::Present(changed());
        }
        ExtensionEvent::Session(SessionEvent::Compact(event)) => {
            event.compaction_entry.summary = changed();
        }
        ExtensionEvent::Session(SessionEvent::BeforeTree(event)) => {
            event.preparation.target_id = changed();
        }
        ExtensionEvent::Session(SessionEvent::Tree(event)) => event.new_leaf_id = Some(changed()),
        ExtensionEvent::Session(SessionEvent::Shutdown(shutdown)) => {
            shutdown.target_session_file = Presence::Present(changed());
        }
        ExtensionEvent::BeforeProviderRequest(request) => request.payload = changed(),
        ExtensionEvent::AfterProviderResponse(response) => response.status = 201.0,
        ExtensionEvent::ToolCall(event) => tool_author::mark(&mut event.input, &event.tool_name),
        ExtensionEvent::ToolResult(event) => event.is_error = !event.is_error,
        ExtensionEvent::UserBash(event) => event.command = changed(),
        ExtensionEvent::Input(input) => input.text = changed(),
    }
}

/// The event a document describes, tagged with its kind; a compaction gets a signal that is
/// never cancelled.
fn event_from(document: Value) -> ExtensionResult<ExtensionEvent> {
    let tag = document["type"].as_str().unwrap_or_default().to_owned();
    let unreadable = |error: serde_json::Error| format!("unreadable event: {error}");
    Ok(match tag.as_str() {
        "model_select" => ExtensionEvent::ModelSelect(from_value(document).map_err(unreadable)?),
        "thinking_level_select" => {
            ExtensionEvent::ThinkingLevelSelect(from_value(document).map_err(unreadable)?)
        }
        "tool_execution_start" => {
            ExtensionEvent::ToolExecutionStart(from_value(document).map_err(unreadable)?)
        }
        "tool_execution_update" => {
            ExtensionEvent::ToolExecutionUpdate(from_value(document).map_err(unreadable)?)
        }
        "tool_execution_end" => {
            ExtensionEvent::ToolExecutionEnd(from_value(document).map_err(unreadable)?)
        }

        "context" => ExtensionEvent::Context(from_value(document).map_err(unreadable)?),
        "before_agent_start" => {
            ExtensionEvent::BeforeAgentStart(from_value(document).map_err(unreadable)?)
        }
        "agent_start" => ExtensionEvent::AgentStart(from_value(document).map_err(unreadable)?),
        "agent_end" => ExtensionEvent::AgentEnd(from_value(document).map_err(unreadable)?),
        "turn_start" => ExtensionEvent::TurnStart(from_value(document).map_err(unreadable)?),
        "turn_end" => ExtensionEvent::TurnEnd(from_value(document).map_err(unreadable)?),
        "message_start" => ExtensionEvent::MessageStart(from_value(document).map_err(unreadable)?),
        "message_update" => {
            ExtensionEvent::MessageUpdate(from_value(document).map_err(unreadable)?)
        }
        "message_end" => ExtensionEvent::MessageEnd(from_value(document).map_err(unreadable)?),
        "resources_discover" => {
            ExtensionEvent::ResourcesDiscover(from_value(document).map_err(unreadable)?)
        }
        "session_start" => ExtensionEvent::Session(SessionEvent::Start(
            from_value(document).map_err(unreadable)?,
        )),
        "session_before_switch" => ExtensionEvent::Session(SessionEvent::BeforeSwitch(
            from_value(document).map_err(unreadable)?,
        )),
        "session_before_fork" => ExtensionEvent::Session(SessionEvent::BeforeFork(
            from_value(document).map_err(unreadable)?,
        )),
        "session_before_compact" => ExtensionEvent::Session(SessionEvent::BeforeCompact(Box::new(
            SessionBeforeCompactEvent {
                data: from_value(document).map_err(unreadable)?,
                signal: AbortSignal::new(Rc::new(Live)),
            },
        ))),
        "session_shutdown" => ExtensionEvent::Session(SessionEvent::Shutdown(
            from_value(document).map_err(unreadable)?,
        )),
        "before_provider_request" => {
            ExtensionEvent::BeforeProviderRequest(from_value(document).map_err(unreadable)?)
        }
        "after_provider_response" => {
            ExtensionEvent::AfterProviderResponse(from_value(document).map_err(unreadable)?)
        }
        "input" => ExtensionEvent::Input(from_value(document).map_err(unreadable)?),
        other => return Err(format!("unknown event kind {other:?}")),
    })
}

/// The result of the family a value describes, whatever event is being handled.
fn reply(family: &str, value: Value) -> ExtensionResult<ExtensionEventResult> {
    let unreadable = |error: serde_json::Error| format!("unreadable result: {error}");
    Ok(match family {
        "tool_call" => ExtensionEventResult::ToolCall(from_value(value).map_err(unreadable)?),
        "tool_result" => ExtensionEventResult::ToolResult(from_value(value).map_err(unreadable)?),
        "user_bash" => ExtensionEventResult::UserBash(from_value(value).map_err(unreadable)?),
        "context" => ExtensionEventResult::Context(from_value(value).map_err(unreadable)?),
        "message_end" => ExtensionEventResult::MessageEnd(from_value(value).map_err(unreadable)?),
        "before_agent_start" => {
            ExtensionEventResult::BeforeAgentStart(from_value(value).map_err(unreadable)?)
        }
        "resources_discover" => {
            ExtensionEventResult::ResourcesDiscover(from_value(value).map_err(unreadable)?)
        }
        "session_before_switch" => {
            ExtensionEventResult::SessionBeforeSwitch(from_value(value).map_err(unreadable)?)
        }
        "session_before_fork" => {
            ExtensionEventResult::SessionBeforeFork(from_value(value).map_err(unreadable)?)
        }
        "session_before_compact" => {
            ExtensionEventResult::SessionBeforeCompact(from_value(value).map_err(unreadable)?)
        }
        "session_before_tree" => {
            ExtensionEventResult::SessionBeforeTree(from_value(value).map_err(unreadable)?)
        }
        "before_provider_request" => {
            ExtensionEventResult::BeforeProviderRequest(from_value(value).map_err(unreadable)?)
        }
        "input" => ExtensionEventResult::Input(from_value(value).map_err(unreadable)?),
        other => return Err(format!("unknown result family {other:?}")),
    })
}

/// Writes a number as an extension would, without a JSON conversion first.
fn write_number(event: &mut ExtensionEvent, bits: &str, field: Option<&str>) -> Result<(), String> {
    let bits = u64::from_str_radix(bits, 16).map_err(|e| e.to_string())?;
    let value = f64::from_bits(bits);
    if matches!(
        event,
        ExtensionEvent::ToolCall(_) | ExtensionEvent::ToolResult(_)
    ) {
        return tool_author::write_number(event, field.unwrap_or("limit"), value);
    }
    match event {
        ExtensionEvent::Context(context) => {
            let Some(maestro_extensions_wasm::AgentMessage::Message(message)) =
                context.messages.first_mut()
            else {
                return Err("missing user message".to_owned());
            };
            let maestro_extensions_wasm::Message::User(user) = message.as_mut() else {
                return Err("not a user message".to_owned());
            };
            user.timestamp = value;
        }
        ExtensionEvent::MessageUpdate(event) => {
            use maestro_extensions_wasm::AssistantMessageEvent as Event;
            let shared = match &event.assistant_message_event {
                Event::Start { partial }
                | Event::TextStart { partial, .. }
                | Event::TextDelta { partial, .. }
                | Event::TextEnd { partial, .. }
                | Event::ThinkingStart { partial, .. }
                | Event::ThinkingDelta { partial, .. }
                | Event::ThinkingEnd { partial, .. }
                | Event::ToolcallStart { partial, .. }
                | Event::ToolcallDelta { partial, .. }
                | Event::ToolcallEnd { partial, .. } => partial,
                Event::Done { message, .. } => message,
                Event::Error { error, .. } => error,
            };
            shared.write().unwrap().usage.total_tokens = value;
        }
        ExtensionEvent::Session(SessionEvent::BeforeCompact(compact)) => match field {
            Some("reserveTokens") => compact.preparation.settings.reserve_tokens = value,
            Some("keepRecentTokens") => compact.preparation.settings.keep_recent_tokens = value,
            _ => compact.preparation.tokens_before = value,
        },
        ExtensionEvent::Session(SessionEvent::Compact(compact)) => {
            compact.compaction_entry.tokens_before = value;
        }
        ExtensionEvent::TurnStart(turn) => turn.timestamp = value,
        _ => return Err("unsupported number edit".to_owned()),
    }
    Ok(())
}

/// Assigns a timestamp through each delivered model role.
fn mark_message(message: &mut maestro_extensions_wasm::AgentMessage) {
    if let maestro_extensions_wasm::AgentMessage::Message(message) = message {
        match message.as_mut() {
            maestro_extensions_wasm::Message::User(user) => user.timestamp = 99.0,
            maestro_extensions_wasm::Message::Assistant(assistant) => assistant.timestamp = 99.0,
            maestro_extensions_wasm::Message::ToolResult(tool) => tool.timestamp = 99.0,
        }
    }
}

/// Apply nested changes through the typed session payloads.
fn edit_session(event: &mut ExtensionEvent, edit: &str) {
    let changed = || "edited Ω".to_owned();
    match event {
        ExtensionEvent::Session(SessionEvent::BeforeCompact(event)) if edit == "nested" => {
            event.preparation.messages_to_summarize.reverse();
            event.branch_entries.reverse();
            event.preparation.file_ops.read.shift_remove("z");
            event.preparation.file_ops.read.insert("z".to_owned());
        }
        ExtensionEvent::Session(SessionEvent::BeforeCompact(event)) => {
            event.preparation.previous_summary = Presence::Present(changed());
        }
        ExtensionEvent::Session(SessionEvent::Compact(event)) => {
            event.compaction_entry.summary = changed();
        }
        ExtensionEvent::Session(SessionEvent::BeforeTree(event)) => {
            event.preparation.target_id = changed();
            if edit == "nested" {
                event.preparation.entries_to_summarize.reverse();
            }
        }
        ExtensionEvent::Session(SessionEvent::Tree(event)) => event.new_leaf_id = Some(changed()),
        _ => {}
    }
}

/// Assemble the example summary without cutting a Unicode scalar value.
fn summarize(event: &ExtensionEvent) -> ExtensionResult<ExtensionEventResult> {
    use maestro_extensions_wasm::{AgentMessage, Message, UserContent};
    let ExtensionEvent::Session(SessionEvent::BeforeCompact(event)) = event else {
        return Err("not a before-compaction event".to_owned());
    };
    let requests = event
        .preparation
        .messages_to_summarize
        .iter()
        .filter_map(|message| {
            let AgentMessage::Message(message) = message else {
                return None;
            };
            let Message::User(user) = message.as_ref() else {
                return None;
            };
            let text = match &user.content {
                UserContent::Text(text) => text.chars().take(100).collect::<String>(),
                UserContent::Blocks(_) => "[complex]".to_owned(),
            };
            Some(format!("- {text}"))
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok(ExtensionEventResult::SessionBeforeCompact(
        SessionBeforeCompactResult {
            cancel: Presence::Missing,
            compaction: Presence::Present(CompactionResult {
                summary: format!("User requests:\n{requests}"),
                first_kept_entry_id: event.preparation.first_kept_entry_id.clone(),
                tokens_before: event.preparation.tokens_before,
                details: Presence::Missing,
            }),
        },
    ))
}

/// Applies the selected callback edits through the typed event payloads.
fn apply_event_edits(event: &mut ExtensionEvent, directive: &Directive) -> ExtensionResult<()> {
    if let Some(value) = &directive.compatibility_edit
        && let ExtensionEvent::ModelSelect(event) = event
    {
        event
            .model
            .compat
            .get_or_insert_with(Default::default)
            .0
            .insert("open".into(), value.clone());
    }

    if directive.session_edit.as_deref() == Some("tool custom")
        && let ExtensionEvent::ToolCall(event) = event
    {
        tool_author::mark_custom(&mut event.input)?;
    }
    if directive.session_edit.as_deref() == Some("tool extra")
        && let ExtensionEvent::ToolCall(event) = event
    {
        tool_author::mark_extra(&mut event.input);
    }
    if let Some(edit) = &directive.session_edit {
        edit_session(event, edit);
    }
    if matches!(directive.session_action, Some(SessionAction::FileEdit))
        && let ExtensionEvent::Session(SessionEvent::BeforeCompact(compact)) = event
        && compact.preparation.file_ops.read.shift_remove("z")
    {
        compact.preparation.file_ops.read.insert("z".to_owned());
    }
    Ok(())
}
