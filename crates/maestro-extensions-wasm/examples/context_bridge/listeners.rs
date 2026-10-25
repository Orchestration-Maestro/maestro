//! Authored listener programs that native ordering witnesses drive step by step.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    CallbackEmitter, EventBus, ExtensionAPI, ExtensionCommandContext, ExtensionFuture,
    ExtensionResult, Subscription,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};

/// One authored step, run in the order the native caller supplies.
#[derive(Clone, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case")]
enum Op {
    /// Subscribes one listener.
    On(Listen),
    /// Removes a stored subscription; repeating it is inert.
    Off {
        /// Subscription to remove.
        id: String,
    },
    /// Drops a stored subscription handle without removing the subscription.
    Forget {
        /// Subscription whose handle is dropped.
        id: String,
    },
    /// Traces, waits for the native caller, then traces its own signal.
    Root {
        /// Name of this invocation in trace entries.
        tag: String,
    },
    /// Emits ordinarily, then reports that the emit returned.
    Emit {
        /// Literal channel.
        channel: String,
        /// Payload.
        data: Value,
    },
    /// Emits, then stays suspended until the native caller completes its wait.
    EmitAndWait {
        /// Literal channel.
        channel: String,
    },
    /// Registers a tool whose execution emits its parameters.
    Tool,
    /// Registers a session-start handler that emits on `channel`.
    Typed {
        /// Literal channel.
        channel: String,
    },
    /// Registers tool `echo`, with or without preparation.
    Echo {
        /// Whether the tool prepares its arguments.
        prepare: bool,
    },
    /// Selects how later preparations answer.
    PrepareMode {
        /// Authored answer.
        mode: String,
    },
    /// Replaces renderer `swap-view` with views named after `version`.
    Swap {
        /// Version in later view names.
        version: String,
    },
    /// Requests compaction with `absent`, `empty` or `full` options.
    Compact {
        /// Options presence.
        options: String,
    },
}

/// An authored listener: what its prefix records, its tail and its one-time edits.
#[derive(Clone, Deserialize)]
struct Listen {
    /// Key for a later removal.
    id: String,
    /// Literal channel.
    channel: String,
    /// Name used in trace entries.
    name: String,
    /// Prefix observation.
    #[serde(default)]
    record: Record,
    /// Continuation returned by the prefix.
    #[serde(default)]
    tail: Tail,
    /// Steps the first delivery performs before it returns.
    #[serde(default)]
    edits: Vec<Op>,
    /// Whether the prefix traces `<name>:end` after its edits.
    #[serde(default)]
    end: bool,
    /// Whether the listener and its tail share a capture that flags its release.
    #[serde(default)]
    retain: bool,
}

/// A capture that registers `dropped-<id>` when its last owner releases it.
struct Release {
    /// Original API, including during the destructor.
    api: ExtensionAPI,
    /// Subscription key in the flag name.
    id: String,
}

impl Drop for Release {
    fn drop(&mut self) {
        let _ = self.api.register_flag(
            &format!("dropped-{}", self.id),
            maestro_extensions_wasm::FlagOptions {
                description: None,
                type_: maestro_extensions_wasm::FlagType::Boolean,
                default: Some(maestro_extensions_wasm::FlagValue::Boolean(true)),
            },
        );
    }
}

/// What a listener prefix reports.
#[derive(Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Record {
    /// Traces `<name>:prefix`.
    #[default]
    Prefix,
    /// Traces the name followed by the payload.
    Data,
    /// Forwards the received payload unchanged.
    Payload,
}

/// The continuation a listener prefix returns.
#[derive(Clone, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum Tail {
    /// No continuation.
    #[default]
    None,
    /// Traces `<name>:tail` once polled.
    Ready,
    /// Waits for another tail to open the instance gate.
    Wait,
    /// Traces, then opens the instance gate.
    Open,
    /// Emits on the channel, then traces.
    Emit(String),
    /// Traces, then lets the suspended emitter resume.
    Done,
}

/// Subscriptions and the gate one instance's programs share.
#[derive(Default)]
struct Programs {
    /// Stored removers by authored key.
    subscriptions: RefCell<HashMap<String, Subscription>>,
    /// Instance-local gate between tails.
    gate: Rc<Gate>,
}

/// A one-shot condition that a waiting tail suspends on.
#[derive(Default)]
struct Gate {
    /// Whether the gate opened.
    open: Cell<bool>,
    /// The suspended waiter.
    waiter: RefCell<Option<Waker>>,
}

impl Gate {
    /// Opens the gate and wakes its waiter.
    fn open(&self) {
        self.open.set(true);
        if let Some(waiter) = self.waiter.take() {
            waiter.wake();
        }
    }
}

/// Waits until a gate opens.
struct Opened(Rc<Gate>);

impl Future for Opened {
    type Output = ();
    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
        if self.0.open.get() {
            Poll::Ready(())
        } else {
            *self.0.waiter.borrow_mut() = Some(context.waker().clone());
            Poll::Pending
        }
    }
}

thread_local! {
    /// This instance's programs, shared by every command call.
    static PROGRAMS: Rc<Programs> = Rc::default();
}

/// Runs each supplied step in order.
pub(super) async fn run(
    api: ExtensionAPI,
    args: &str,
    context: ExtensionCommandContext,
) -> ExtensionResult<()> {
    let programs = PROGRAMS.with(Rc::clone);
    let ops: Vec<Op> = serde_json::from_str(args).map_err(|error| error.to_string())?;
    for op in ops {
        match op {
            Op::EmitAndWait { channel } => {
                emit(&api, &channel, Value::Null).await?;
                context.wait_for_idle().await?;
                trace(&api.events(), &api, "emitter:resumed").await?;
            }
            Op::Compact { options } => super::results::compact(&api, &context, &options).await?,
            Op::Root { tag } => {
                trace(&api.events(), &api, &format!("{tag}:start")).await?;
                let signal = context.signal()?.ok_or("missing root signal")?;
                context.wait_for_idle().await?;
                let done = format!("{tag}:done:{}", signal.aborted());
                trace(&api.events(), &api, &done).await?;
            }
            op => step(&api, &programs, op).await?,
        }
    }
    Ok(())
}

/// Runs one step that needs no command context.
async fn step(api: &ExtensionAPI, programs: &Rc<Programs>, op: Op) -> ExtensionResult<()> {
    match op {
        Op::Emit { channel, data } => emit(api, &channel, data).await,
        Op::Tool => tool(api),
        Op::Typed { channel } => typed(api, channel),
        Op::Echo { prepare } => super::results::echo(api, prepare),
        Op::Swap { version } => super::components::swap(api, version),
        Op::PrepareMode { mode } => {
            super::results::set_mode(mode);
            Ok(())
        }
        op => edit(api, programs, op),
    }
}

/// Applies a step inside a listener, emitting through its callback emitter.
fn apply(
    api: &ExtensionAPI,
    programs: &Rc<Programs>,
    emitter: &CallbackEmitter<'_>,
    op: Op,
) -> ExtensionResult<()> {
    match op {
        Op::Emit { channel, data } => emitter.emit(&channel, data),
        op => edit(api, programs, op),
    }
}

/// Applies a synchronous subscription edit.
fn edit(api: &ExtensionAPI, programs: &Rc<Programs>, op: Op) -> ExtensionResult<()> {
    match op {
        Op::On(listen) => {
            let id = listen.id.clone();
            let subscription = api
                .events()
                .on(&listen.channel.clone(), listener(api, programs, listen))?;
            programs.subscriptions.borrow_mut().insert(id, subscription);
            Ok(())
        }
        Op::Forget { id } => {
            let forgotten = programs.subscriptions.borrow_mut().remove(&id);
            forgotten
                .map(drop)
                .ok_or_else(|| format!("unknown subscription {id}"))
        }
        Op::Off { id } => {
            let subscription = programs.subscriptions.borrow().get(&id).cloned();
            subscription
                .ok_or_else(|| format!("unknown subscription {id}"))?
                .unsubscribe()
        }
        _ => Err("only subscription edits run inside a listener".into()),
    }
}

/// Builds a listener whose first delivery applies its edits.
fn listener(
    api: &ExtensionAPI,
    programs: &Rc<Programs>,
    listen: Listen,
) -> maestro_extensions_wasm::EventListener {
    let (api, programs, pending) = (api.clone(), programs.clone(), Cell::new(true));
    let release = listen.retain.then(|| {
        Rc::new(Release {
            api: api.clone(),
            id: listen.id.clone(),
        })
    });
    Rc::new(move |data, emitter| {
        prefix(&api, &listen, data, &emitter)?;
        if pending.replace(false) {
            for op in listen.edits.clone() {
                apply(&api, &programs, &emitter, op)?;
            }
        }
        if listen.end {
            let label = super::ordering::label(&api)?;
            emitter.emit(
                &format!("trace:{label}"),
                json!(format!("{}:end", listen.name)),
            )?;
        }
        let retained = release.clone();
        Ok(continuation(&api, &programs, &listen).map(|tail| {
            Box::pin(async move {
                let result = tail.await;
                drop(retained);
                result
            }) as ExtensionFuture<'static, ()>
        }))
    })
}

/// Reports the listener's prefix through its callback emitter.
fn prefix(
    api: &ExtensionAPI,
    listen: &Listen,
    data: Value,
    emitter: &CallbackEmitter<'_>,
) -> ExtensionResult<()> {
    let label = super::ordering::label(api)?;
    match listen.record {
        Record::Prefix => emitter.emit(
            &format!("trace:{label}"),
            json!(format!("{}:prefix", listen.name)),
        ),
        Record::Data => emitter.emit(
            &format!("trace:{label}"),
            json!(format!("{}{data}", listen.name)),
        ),
        Record::Payload => emitter.emit(&format!("payload:{label}"), data),
    }
}

/// Creates the listener's authored continuation.
fn continuation(
    api: &ExtensionAPI,
    programs: &Programs,
    listen: &Listen,
) -> Option<ExtensionFuture<'static, ()>> {
    let (events, api, gate) = (api.events(), api.clone(), programs.gate.clone());
    let step = format!("{}:tail", listen.name);
    let tail = listen.tail.clone();
    if matches!(tail, Tail::None) {
        return None;
    }
    Some(Box::pin(async move {
        match &tail {
            Tail::Wait => Opened(gate.clone()).await,
            Tail::Emit(channel) => events.emit(channel, Value::Null).await?,
            _ => {}
        }
        trace(&events, &api, &step).await?;
        match tail {
            Tail::Open => gate.open(),
            Tail::Done => events.emit("done", Value::Null).await?,
            _ => {}
        }
        Ok(())
    }))
}

/// Emits ordinarily, then reports the return on this instance's trace channel.
async fn emit(api: &ExtensionAPI, channel: &str, data: Value) -> ExtensionResult<()> {
    let events = api.events();
    events.emit(channel, data).await?;
    trace(&events, api, "emit:return").await
}

/// Traces one step outside a callback.
async fn trace(events: &EventBus, api: &ExtensionAPI, step: &str) -> ExtensionResult<()> {
    let label = super::ordering::label(api)?;
    events.emit(&format!("trace:{label}"), json!(step)).await
}

/// Registers a tool whose execution emits `{channel, data}` parameters.
fn tool(api: &ExtensionAPI) -> ExtensionResult<()> {
    let tool_api = api.clone();
    api.register_tool(maestro_extensions_wasm::ToolDefinition {
        metadata: serde_json::from_value(json!({"name":"bridge-emit", "label":"Emit", "description":"Emits its parameters", "parameters":{"type":"object"}})).map_err(|error| error.to_string())?,
        prepare_arguments: None,
        execute: Rc::new(move |_, params, _, _, _| {
            let api = tool_api.clone();
            Box::pin(async move {
                let channel = params["channel"].as_str().ok_or("missing channel")?.to_owned();
                emit(&api, &channel, params["data"].clone()).await?;
                serde_json::from_value(json!({"content":[],"details":null})).map_err(|error| error.to_string())
            })
        }),
    })
}

/// Registers a session-start handler that emits ordinarily on `channel`.
fn typed(api: &ExtensionAPI, channel: String) -> ExtensionResult<()> {
    let handler_api = api.clone();
    api.on(
        "session_start",
        Rc::new(move |_, _| {
            let (api, channel) = (handler_api.clone(), channel.clone());
            Box::pin(async move {
                emit(&api, &channel, Value::Null).await?;
                Ok(None)
            })
        }),
    )
}
