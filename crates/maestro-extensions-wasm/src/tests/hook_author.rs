//! Author extension whose hooks are described by the test that drives it.
//!
//! The factory registers one command, `hooks`. Each invocation installs the handlers or
//! registrations its JSON request describes, so one built component serves every recorded hook
//! case. Handlers report what they observe on the `trace` bus channel. The installed
//! directory selects a failing initialization, so loading tests need no second component.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use maestro_extensions_wasm::{
    BashExecOptions, BashExecResult, BashOperations, CommandOptions, ExtensionAPI,
    ExtensionCommandContext, ExtensionContext, ExtensionEvent, ExtensionEventResult,
    ExtensionFuture, ExtensionResult, JsonText, Presence, SessionEvent, ToolInput,
    UserBashEventResult,
};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

#[path = "hook_registrations.rs"]
mod hook_registrations;

/// Variable through which the host names the artifact this instance was loaded from.
const LOCATION: &str = "MAESTRO_EXTENSION_PATH";

/// One request to the `hooks` command.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
enum Request {
    /// Registers each handler in order.
    Handlers(Vec<Handler>),
    /// Reports how often this instance has counted.
    Count,
    /// Registers one labelled set of commands, tools, renderers, shortcuts, flags and handlers.
    Registrations(hook_registrations::Registrations),
}

/// A handler for one event name and the steps it runs.
#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
struct Handler {
    /// Event name, forwarded unchanged.
    event: String,
    /// Steps in order; the first ending step ends the handler.
    steps: Vec<Step>,
}

/// One action of a described handler.
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
enum Step {
    /// Reports the value on the trace channel.
    Trace(Value),
    /// Reports a value read from the event or its context.
    Observe(Observation),
    /// Waits for idle on the command context of the latest `hooks` invocation.
    Wait,
    /// Asks the context to abort.
    Abort,
    /// Replaces one field of the event.
    Edit(Edit),
    /// Fails with this message.
    Fail(String),
    /// Returns this document as the event's result.
    Return(Value),
    /// Returns the event's own message, payload or messages.
    Echo,
    /// Returns shell operations that report their invocation.
    Operations,
    /// Skips the next steps unless the agent start carries this prompt.
    SkipUnless {
        /// The prompt that runs the skipped steps.
        prompt: String,
        /// How many steps to skip.
        steps: usize,
    },
    /// Appends text to the event's prompt, input or tool-result content.
    Suffix(String),
}

/// A value a handler reads.
#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
enum Observation {
    /// The content of the first context message.
    FirstContent,
    /// The provider payload.
    Payload,
    /// The event's system prompt and the context's prompt reader.
    Prompts,
    /// The context's prompt reader alone.
    ContextPrompt,
    /// The prompt, images and prompt options of an agent start.
    Agent,
    /// The input text, images and source.
    Input,
    /// The tool call's path argument.
    ToolPath,
    /// Whether the event's signal is aborted.
    Aborted,
}

/// A field replacement, decoded into the field's own type.
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
enum Edit {
    /// Assigns the context messages.
    Messages(Value),
    /// Appends one context message.
    Append(Value),
    /// Assigns the provider payload.
    Payload(Value),
    /// Assigns the tool call input.
    ToolInput(Value),
    /// Assigns the input images.
    Images(Value),
}

/// What the handlers and the command share.
#[derive(Clone)]
struct Hooks {
    /// Registers and reports.
    api: ExtensionAPI,
    /// The command context of the latest `hooks` invocation.
    captured: Rc<RefCell<Option<ExtensionCommandContext>>>,
    /// Invocations of the counting request.
    count: Rc<Cell<u32>>,
}

/// The extension factory: fails as its installed directory selects, else registers `hooks`.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let location = std::env::var(LOCATION).unwrap_or_default();
        let directory = location.rsplit(['/', '\\']).nth(1).unwrap_or_default();
        match directory {
            "fail-empty" => return Err(String::new()),
            "fail-lines" => return Err("first line\nsecond line".into()),
            "fail-unicode" => return Err("échec Ω ✓".into()),
            #[cfg(target_arch = "wasm32")]
            "fail-trap" => std::arch::wasm32::unreachable(),
            _ => {}
        }
        let hooks = Hooks {
            api: api.clone(),
            captured: Rc::default(),
            count: Rc::default(),
        };
        api.register_command(
            "hooks",
            CommandOptions {
                description: None,
                get_argument_completions: None,
                handler: Rc::new(move |args, ctx| {
                    let hooks = hooks.clone();
                    Box::pin(async move { hooks.request(&args, ctx).await })
                }),
            },
        )
    })
}

impl Hooks {
    /// Runs one `hooks` request, keeping its command context for later waits.
    async fn request(&self, args: &str, ctx: ExtensionCommandContext) -> ExtensionResult<()> {
        let request: Request = serde_json::from_str(args).map_err(|e| e.to_string())?;
        self.captured.replace(Some(ctx));
        match request {
            Request::Handlers(handlers) => handlers
                .into_iter()
                .try_for_each(|handler| self.register(&handler.event, handler.steps)),
            Request::Count => {
                self.count.set(self.count.get() + 1);
                self.trace(json!({ "count": self.count.get() })).await
            }
            Request::Registrations(registrations) => registrations.register(&self.api),
        }
    }

    /// Registers a handler that runs `steps` for `event`.
    fn register(&self, event: &str, steps: Vec<Step>) -> ExtensionResult<()> {
        let hooks = self.clone();
        let steps: Rc<[Step]> = steps.into();
        self.api.on(
            event,
            Rc::new(move |event, ctx| {
                let (hooks, steps) = (hooks.clone(), Rc::clone(&steps));
                Box::pin(async move { hooks.run(&steps, event, ctx).await })
            }),
        )
    }

    /// Reports a value on the trace channel.
    async fn trace(&self, value: Value) -> ExtensionResult<()> {
        let events = self.api.events();
        events.emit("trace", value).await
    }

    /// Runs the steps until one ends the handler.
    async fn run(
        &self,
        steps: &[Step],
        event: &mut ExtensionEvent,
        ctx: ExtensionContext,
    ) -> ExtensionResult<Option<ExtensionEventResult>> {
        let mut skipped = 0;
        for step in steps {
            if skipped > 0 {
                skipped -= 1;
                continue;
            }
            match step {
                Step::Trace(value) => self.trace(value.clone()).await?,
                Step::Observe(observation) => {
                    self.trace(observe(*observation, event, &ctx)?).await?;
                }
                Step::Wait => {
                    let command = self.captured.borrow().clone();
                    command.ok_or("no command context")?.wait_for_idle().await?;
                }
                Step::Abort => ctx.abort()?,
                Step::Edit(edit) => apply(edit, event)?,
                Step::Fail(message) => return Err(message.clone()),
                Step::Return(value) => return result(event, value.clone()).map(Some),
                Step::Echo => return echo(event).map(Some),
                Step::Operations => return Ok(Some(self.operations())),
                Step::SkipUnless { prompt, steps } => skipped = skipping(event, prompt, *steps),
                Step::Suffix(suffix) => return suffixed(event, suffix).map(Some),
            }
        }
        Ok(None)
    }

    /// Shell operations that check their operands, stream one chunk and report the call.
    fn operations(&self) -> ExtensionEventResult {
        let hooks = self.clone();
        ExtensionEventResult::UserBash(UserBashEventResult {
            result: Presence::Missing,
            operations: Some(BashOperations {
                exec: Rc::new(move |command, cwd, options| {
                    Box::pin(hooks.clone().exec(command, cwd, options))
                }),
            }),
        })
    }

    /// Checks the supplied operands, streams one chunk and reports the invocation.
    async fn exec(
        self,
        command: String,
        cwd: String,
        options: BashExecOptions,
    ) -> ExtensionResult<BashExecResult> {
        if command != "controlled" || cwd != "/work" {
            return Err(format!("unexpected shell operands {command} {cwd}"));
        }
        (options.on_data)(b"bytes")?;
        self.trace(json!("executed")).await?;
        Ok(BashExecResult {
            exit_code: Some(0.0),
        })
    }
}

/// Decodes a document into a field or result type.
fn decoded<T: DeserializeOwned>(value: Value) -> ExtensionResult<T> {
    serde_json::from_value(value).map_err(|e| e.to_string())
}

/// Encodes a field as JSON.
fn encoded<T: serde::Serialize>(value: &T) -> ExtensionResult<Value> {
    serde_json::to_value(value).map_err(|e| e.to_string())
}

/// Reads the observed value.
fn observe(
    observation: Observation,
    event: &ExtensionEvent,
    ctx: &ExtensionContext,
) -> ExtensionResult<Value> {
    match (observation, event) {
        (Observation::FirstContent, ExtensionEvent::Context(context)) => {
            let first = context.messages.first().ok_or("no context message")?;
            Ok(encoded(first)?["content"].clone())
        }
        (Observation::Payload, ExtensionEvent::BeforeProviderRequest(request)) => {
            serde_json::from_str(&request.payload).map_err(|e| e.to_string())
        }
        (Observation::Prompts, ExtensionEvent::BeforeAgentStart(start)) => {
            Ok(json!([start.system_prompt, ctx.get_system_prompt()?]))
        }
        (Observation::ContextPrompt, _) => Ok(json!(ctx.get_system_prompt()?)),
        (Observation::Agent, ExtensionEvent::BeforeAgentStart(start)) => Ok(json!({
            "prompt": start.prompt,
            "images": encoded(&start.images)?,
            "options": encoded(&start.system_prompt_options)?,
        })),
        (Observation::Input, ExtensionEvent::Input(input)) => encoded(input),
        (Observation::ToolPath, ExtensionEvent::ToolCall(call)) => {
            Ok(call.input.decode::<Value>().map_err(|e| e.to_string())?["path"].clone())
        }
        (Observation::Aborted, ExtensionEvent::Session(SessionEvent::BeforeCompact(compact))) => {
            Ok(json!(compact.signal.aborted()))
        }
        (Observation::Aborted, ExtensionEvent::Session(SessionEvent::BeforeTree(tree))) => {
            Ok(json!(tree.signal.aborted()))
        }
        _ => Err("observation does not apply to this event".into()),
    }
}

/// Applies a field replacement to the event it belongs to.
fn apply(edit: &Edit, event: &mut ExtensionEvent) -> ExtensionResult<()> {
    match (edit, event) {
        (Edit::Messages(value), ExtensionEvent::Context(context)) => {
            context.messages = decoded(value.clone())?;
        }
        (Edit::Append(value), ExtensionEvent::Context(context)) => {
            context.messages.push(decoded(value.clone())?);
        }
        (Edit::Payload(value), ExtensionEvent::BeforeProviderRequest(request)) => {
            request.payload = JsonText::from(value.to_string());
        }
        (Edit::ToolInput(value), ExtensionEvent::ToolCall(call)) => {
            call.input = ToolInput::from_value(value)?;
        }
        (Edit::Images(value), ExtensionEvent::Input(input)) => {
            input.images = decoded(value.clone())?;
        }
        _ => return Err("edit does not apply to this event".into()),
    }
    Ok(())
}

/// The document as the result type of the handled event; a notification's result is unused.
fn result(event: &ExtensionEvent, value: Value) -> ExtensionResult<ExtensionEventResult> {
    use ExtensionEventResult as Result;
    Ok(match event {
        ExtensionEvent::Context(_) => Result::Context(decoded(value)?),
        ExtensionEvent::MessageEnd(_) => Result::MessageEnd(decoded(value)?),
        ExtensionEvent::BeforeAgentStart(_) => Result::BeforeAgentStart(decoded(value)?),
        ExtensionEvent::ToolCall(_) => Result::ToolCall(decoded(value)?),
        ExtensionEvent::ToolResult(_) => Result::ToolResult(decoded(value)?),
        ExtensionEvent::UserBash(_) => Result::UserBash(decoded(value)?),
        ExtensionEvent::ResourcesDiscover(_) => Result::ResourcesDiscover(decoded(value)?),
        ExtensionEvent::Input(_) => Result::Input(decoded(value)?),
        ExtensionEvent::BeforeProviderRequest(_) => {
            Result::BeforeProviderRequest(JsonText::from(value.to_string()))
        }
        ExtensionEvent::Session(SessionEvent::BeforeFork(_)) => {
            Result::SessionBeforeFork(decoded(value)?)
        }
        ExtensionEvent::Session(SessionEvent::BeforeCompact(_)) => {
            Result::SessionBeforeCompact(decoded(value)?)
        }
        ExtensionEvent::Session(SessionEvent::BeforeTree(_)) => {
            Result::SessionBeforeTree(decoded(value)?)
        }
        _ => Result::SessionBeforeSwitch(decoded(value)?),
    })
}

/// How many steps to skip: none for an agent start carrying `prompt`, else `steps`.
fn skipping(event: &ExtensionEvent, prompt: &str, steps: usize) -> usize {
    match event {
        ExtensionEvent::BeforeAgentStart(start) if start.prompt == prompt => 0,
        _ => steps,
    }
}

/// Appends `suffix` to the prompt, input text or tool-result content and returns it.
fn suffixed(event: &ExtensionEvent, suffix: &str) -> ExtensionResult<ExtensionEventResult> {
    let value = match event {
        ExtensionEvent::BeforeAgentStart(start) => {
            json!({ "systemPrompt": format!("{}{suffix}", start.system_prompt) })
        }
        ExtensionEvent::Input(input) => {
            json!({ "action": "transform", "text": format!("{}{suffix}", input.text) })
        }
        ExtensionEvent::ToolResult(tool) => {
            let mut content = tool.content.clone();
            content.push(decoded(json!({ "type": "text", "text": suffix }))?);
            json!({ "content": content })
        }
        _ => return Err("event has no text to extend".into()),
    };
    result(event, value)
}

/// The event's own message, payload or messages, returned as its result.
fn echo(event: &ExtensionEvent) -> ExtensionResult<ExtensionEventResult> {
    use ExtensionEventResult as Result;
    use maestro_extensions_wasm::{ContextEventResult, MessageEndEventResult};
    Ok(match event {
        ExtensionEvent::MessageEnd(end) => Result::MessageEnd(MessageEndEventResult {
            message: Presence::Present(end.message.clone()),
        }),
        ExtensionEvent::BeforeProviderRequest(request) => {
            Result::BeforeProviderRequest(request.payload.clone())
        }
        ExtensionEvent::Context(context) => Result::Context(ContextEventResult {
            messages: Presence::Present(context.messages.clone()),
        }),
        _ => return Err("nothing to echo for this event".into()),
    })
}
