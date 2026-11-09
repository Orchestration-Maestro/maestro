//! Host phases the authored handlers and the originating frame observe around a replacement.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    EventBus, ExtensionAPI, ExtensionCommandContext, ExtensionEvent, ExtensionEventResult,
    ExtensionHandler, ExtensionResult, FlagOptions, FlagType, FlagValue, ForkData, ForkOptions,
    ForkPosition, NewSessionCommandData, NewSessionCommandOptions, SessionEvent, SetupSession,
    SwitchSessionOptions, WithSession,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::rc::Rc;

/// Channel every handler and callback announces the phase it received on.
const CHANNEL: &str = "retention-phase";

/// The flag that makes a generation hear resource discovery.
const HOOKS: &str = "retention-hooks";

/// What a handler answers once it announced its event.
#[derive(Clone, Copy)]
enum Outcome {
    /// Returns no verdict.
    Pass,
    /// Fails with the literal handler failure.
    Fail,
    /// Cancels the switch.
    VetoSwitch,
    /// Cancels the fork.
    VetoFork,
}

/// Registers the discovery handler only for a generation whose flags request it.
pub(super) fn register(api: &ExtensionAPI) -> ExtensionResult<()> {
    api.register_flag(
        HOOKS,
        FlagOptions {
            description: None,
            type_: FlagType::Boolean,
            default: None,
        },
    )?;
    if matches!(api.get_flag(HOOKS)?, Some(FlagValue::Boolean(true))) {
        api.on(
            "resources_discover",
            handler(&api.events(), "discover", Outcome::Pass),
        )?;
    }
    Ok(())
}

/// Runs one replacement or reload request and reports how the originating frame resumed.
pub(super) async fn command(
    api: &ExtensionAPI,
    context: &ExtensionCommandContext,
    request: &str,
) -> ExtensionResult<Value> {
    let mut words = request.split_whitespace();
    let operation = words.next().unwrap_or_default();
    let modes: Vec<&str> = words.collect();
    if operation == "flag" {
        return Ok(json!(matches!(
            api.get_flag(HOOKS)?,
            Some(FlagValue::Boolean(true))
        )));
    }
    install(api, &modes)?;
    let result = run(&api.events(), context, operation).await;
    let mut report = match result {
        Ok(value) => json!({ "ok": value }),
        Err(error) => json!({ "err": error }),
    };
    report["cwd"] = match context.cwd() {
        Ok(cwd) => json!({ "ok": cwd }),
        Err(error) => json!({ "err": error }),
    };
    Ok(report)
}

/// Registers the handlers the request selects, in the order the host walks them.
fn install(api: &ExtensionAPI, modes: &[&str]) -> ExtensionResult<()> {
    let events = api.events();
    if modes.contains(&"shutdown") {
        let failing = if modes.contains(&"shutdown-fails") {
            Outcome::Fail
        } else {
            Outcome::Pass
        };
        for (label, outcome) in [
            ("shutdown-first", Outcome::Pass),
            ("shutdown-second", failing),
            ("shutdown-third", Outcome::Pass),
        ] {
            api.on("session_shutdown", handler(&events, label, outcome))?;
        }
    }
    if modes.contains(&"before") {
        let verdict = |mode, outcome| {
            if modes.contains(&mode) {
                outcome
            } else {
                Outcome::Pass
            }
        };
        let switch = verdict("veto-switch", Outcome::VetoSwitch);
        let fork = verdict("veto-fork", Outcome::VetoFork);
        api.on(
            "session_before_switch",
            handler(&events, "before-switch", switch),
        )?;
        api.on("session_before_fork", handler(&events, "before-fork", fork))?;
    }
    Ok(())
}

/// Starts the selected operation with callbacks that announce when the host runs them.
async fn run(
    events: &EventBus,
    context: &ExtensionCommandContext,
    operation: &str,
) -> ExtensionResult<Value> {
    let changed = |result: maestro_extensions_wasm::SessionChangeResult| json!({ "cancelled": result.cancelled });
    match operation {
        "new" => context
            .new_session(Some(NewSessionCommandOptions {
                setup: Some(setup(events)),
                data: NewSessionCommandData {
                    parent_session: None,
                },
                with_session: Some(continuation(events)),
            }))
            .await
            .map(changed),
        "switch" => context
            .switch_session(
                "/sessions/phases.jsonl",
                Some(SwitchSessionOptions {
                    with_session: Some(continuation(events)),
                }),
            )
            .await
            .map(changed),
        "fork" => context
            .fork(
                "entry-phases",
                Some(ForkOptions {
                    data: ForkData {
                        position: Some(ForkPosition::At),
                    },
                    with_session: Some(continuation(events)),
                }),
            )
            .await
            .map(changed),
        "reload" => context.reload().await.map(|()| json!({ "reloaded": true })),
        other => Err(format!("unknown phase operation: {other}")),
    }
}

/// Announces the writable setup the host runs on the replacement session.
fn setup(events: &EventBus) -> SetupSession {
    let events = events.clone();
    Box::new(move |_| {
        Box::pin(async move { events.emit(CHANNEL, json!({ "callback": "setup" })).await })
    })
}

/// Announces the fresh context the host hands the continuation.
fn continuation(events: &EventBus) -> WithSession {
    let events = events.clone();
    Box::new(move |replaced| {
        Box::pin(async move {
            events
                .emit(CHANNEL, json!({ "continuation": replaced.cwd()? }))
                .await
        })
    })
}

/// Builds a handler that announces its event operands before it answers.
fn handler(events: &EventBus, label: &'static str, outcome: Outcome) -> ExtensionHandler {
    let events = events.clone();
    Rc::new(move |event, _| {
        let events = events.clone();
        Box::pin(async move {
            let operands = operands(event)?;
            events
                .emit(CHANNEL, json!({ "handler": label, "event": operands }))
                .await?;
            Ok(match outcome {
                Outcome::Pass => None,
                Outcome::Fail => return Err("literal shutdown failure".into()),
                Outcome::VetoSwitch => Some(ExtensionEventResult::SessionBeforeSwitch(decode(
                    json!({ "cancel": true }),
                )?)),
                Outcome::VetoFork => Some(ExtensionEventResult::SessionBeforeFork(decode(
                    json!({ "cancel": true }),
                )?)),
            })
        })
    })
}

/// Encodes the exact record the host supplied to a session handler.
fn operands(event: &ExtensionEvent) -> ExtensionResult<Value> {
    match event {
        ExtensionEvent::Session(SessionEvent::Shutdown(event)) => encode(event),
        ExtensionEvent::Session(SessionEvent::BeforeSwitch(event)) => encode(event),
        ExtensionEvent::Session(SessionEvent::BeforeFork(event)) => encode(event),
        ExtensionEvent::ResourcesDiscover(event) => encode(event),
        _ => Err("unexpected phase event".into()),
    }
}

/// Serializes a received record without reshaping it.
fn encode(value: &impl Serialize) -> ExtensionResult<Value> {
    serde_json::to_value(value).map_err(|error| error.to_string())
}

/// Builds an authored verdict from its public record form.
fn decode<T: DeserializeOwned>(value: Value) -> ExtensionResult<T> {
    serde_json::from_value(value).map_err(|error| error.to_string())
}
