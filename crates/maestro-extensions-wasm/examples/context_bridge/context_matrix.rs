//! Individual context operations observed by the native component caller.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{ExtensionCommandContext, ExtensionContext};
use serde_json::{Value, json};
use std::{cell::Cell, rc::Rc};

#[path = "context_matrix/retained.rs"]
mod retained;
pub(super) use retained::register;

/// Performs one operation so native tests can distinguish its result and side effects.
pub(super) async fn observe(
    context: &ExtensionCommandContext,
    request: &str,
) -> Result<Value, String> {
    let (request, checked) = request
        .strip_suffix(" checked")
        .map_or((request, false), |request| (request, true));
    let (member, options) = request.split_once(' ').unwrap_or((request, "none"));
    let callbacks = Rc::new(Cell::new(0));
    let result = match member {
        "waitForIdle" => context.wait_for_idle().await.map(|()| Value::Null),
        "newSession" => context
            .new_session(new_options(options, &callbacks))
            .await
            .map(|r| json!({"cancelled": r.cancelled})),
        "fork" => {
            let entry = if options == "raw" {
                "  e\u{feff}\u{85} "
            } else {
                "entry-1"
            };
            context
                .fork(entry, fork_options(options, &callbacks))
                .await
                .map(|r| json!({"cancelled": r.cancelled}))
        }
        "navigateTree" => context
            .navigate_tree("target-1", navigation_options(options))
            .await
            .map(|r| json!({"cancelled": r.cancelled})),
        "switchSession" => context
            .switch_session("/sessions/s.jsonl", switch_options(options, &callbacks))
            .await
            .map(|r| json!({"cancelled": r.cancelled})),
        "reload" => context.reload().await.map(|()| Value::Null),
        _ => return base(context, member, options),
    };
    if checked {
        Ok(json!({"result":result,"callbacks":callbacks.get()}))
    } else {
        result.map(|value| json!({"value": value, "callbacks": callbacks.get()}))
    }
}

/// Observes a matrix request carried by an ordinary session-start event.
pub(super) fn start(
    context: &ExtensionContext,
    previous: &maestro_extensions_wasm::Presence<String>,
) -> Option<Result<Value, String>> {
    let maestro_extensions_wasm::Presence::Present(request) = previous else {
        return None;
    };
    let request = request.strip_prefix("context matrix ")?;
    let (member, options) = request.split_once(' ').unwrap_or((request, "none"));
    Some(base(context, member, options))
}

/// Reads base imports independently, including methods whose unbound result is absence.
pub(super) fn base(
    context: &ExtensionContext,
    member: &str,
    options: &str,
) -> Result<Value, String> {
    match member {
        "ui" => {
            context.ui()?.notify("matrix notification", None)?;
            Ok(Value::Null)
        }
        "hasUI" => Ok(json!(context.has_ui()?)),
        "cwd" => Ok(json!(context.cwd()?)),
        "sessionManager" => {
            let history = context.session_manager()?;
            Ok(json!({"cwd":history.get_cwd()?,"id":history.get_session_id()?}))
        }
        "modelRegistry" => Ok(json!(context.model_registry()?.get_all()?)),
        "model" => Ok(json!(context.model()?)),
        "isIdle" => Ok(json!(context.is_idle()?)),
        "signal" => Ok(json!(context.signal()?.map(|signal| signal.aborted()))),
        "retainedSignal" => {
            let signal = context.signal()?.ok_or("missing matrix signal")?;
            let before = signal.aborted();
            context.abort()?;
            Ok(json!({"before":before,"after":signal.aborted()}))
        }
        "abort" => {
            context.abort()?;
            Ok(Value::Null)
        }
        "hasPendingMessages" => Ok(json!(context.has_pending_messages()?)),
        "shutdown" => {
            context.shutdown()?;
            Ok(Value::Null)
        }
        "getContextUsage" => Ok(json!(context.get_context_usage()?.map(|usage| json!({
            "tokens":usage.tokens,"contextWindow":usage.context_window,"percent":usage.percent,
        })))),
        "compact" => {
            context.compact(compact_options(options))?;
            Ok(Value::Null)
        }
        "getSystemPrompt" => Ok(json!(context.get_system_prompt()?)),
        _ => Err(format!("unknown matrix member: {member}")),
    }
}

/// Counts authored callbacks even if an unbound operation ignores their result.
fn continuation(callbacks: &Rc<Cell<u32>>) -> maestro_extensions_wasm::WithSession {
    let callbacks = callbacks.clone();
    Box::new(move |_| {
        Box::pin(async move {
            callbacks.set(callbacks.get() + 1);
            Ok(())
        })
    })
}

/// Distinguishes absent, empty and full new-session options.
fn new_options(
    options: &str,
    callbacks: &Rc<Cell<u32>>,
) -> Option<maestro_extensions_wasm::NewSessionCommandOptions> {
    if options == "none" {
        return None;
    }
    let full = options == "full";
    let setup = callbacks.clone();
    Some(maestro_extensions_wasm::NewSessionCommandOptions {
        data: maestro_extensions_wasm::NewSessionCommandData {
            parent_session: full.then(|| "/sessions/parent.jsonl".into()),
        },
        setup: full.then(|| {
            Box::new(
                move |_| -> maestro_extensions_wasm::ExtensionFuture<'static, ()> {
                    Box::pin(async move {
                        setup.set(setup.get() + 1);
                        Ok(())
                    })
                },
            ) as maestro_extensions_wasm::SetupSession
        }),
        with_session: if options == "retired" {
            Some(retired_continuation())
        } else {
            full.then(|| continuation(callbacks))
        },
    })
}

/// Reads a retired replacement while its separately owned message operations remain usable.
fn retired_continuation() -> maestro_extensions_wasm::WithSession {
    Box::new(move |context| {
        Box::pin(async move {
            let cwd = context.cwd();
            let wait = context.wait_for_idle().await;
            let custom = context
                .send_message(
                    serde_json::from_value(json!({
                        "customType":"retired", "content":"note", "display":false,
                    }))
                    .map_err(|error| error.to_string())?,
                    None,
                )
                .await;
            let user = context
                .send_user_message(
                    maestro_extensions_wasm::UserContent::Text("note".into()),
                    None,
                )
                .await;
            Err(json!({"cwd":cwd,"wait":wait,"custom":custom,"user":user}).to_string())
        })
    })
}

/// Carries raw fork positions and the authored continuation independently.
fn fork_options(
    options: &str,
    callbacks: &Rc<Cell<u32>>,
) -> Option<maestro_extensions_wasm::ForkOptions> {
    use maestro_extensions_wasm::ForkPosition;
    if options == "none" || options == "raw" {
        return None;
    }
    Some(maestro_extensions_wasm::ForkOptions {
        data: maestro_extensions_wasm::ForkData {
            position: match options {
                "before" => Some(ForkPosition::Before),
                "full" | "at" => Some(ForkPosition::At),
                _ => None,
            },
        },
        with_session: (options == "full").then(|| continuation(callbacks)),
    })
}

/// Supplies every optional navigation field without trimming authored strings.
fn navigation_options(options: &str) -> Option<maestro_extensions_wasm::NavigateTreeOptions> {
    if options == "none" {
        return None;
    }
    Some(if options == "empty" {
        maestro_extensions_wasm::NavigateTreeOptions::default()
    } else {
        maestro_extensions_wasm::NavigateTreeOptions {
            summarize: Some(true),
            custom_instructions: Some("focus".into()),
            replace_instructions: Some(false),
            label: Some("checkpoint".into()),
        }
    })
}

/// Preserves outer presence independently of the continuation.
fn switch_options(
    options: &str,
    callbacks: &Rc<Cell<u32>>,
) -> Option<maestro_extensions_wasm::SwitchSessionOptions> {
    (options != "none").then(|| maestro_extensions_wasm::SwitchSessionOptions {
        with_session: (options == "full").then(|| continuation(callbacks)),
    })
}

/// Carries original notification callbacks alongside literal instructions.
fn compact_options(options: &str) -> Option<maestro_extensions_wasm::CompactOptions> {
    if options == "none" {
        return None;
    }
    if options == "empty" {
        return Some(maestro_extensions_wasm::CompactOptions::default());
    }
    Some(maestro_extensions_wasm::CompactOptions {
        custom_instructions: Some("keep".into()),
        on_complete: Some(Rc::new(|result, emitter| {
            emitter.emit("matrix-compaction", json!({"complete": result}))
        })),
        on_error: Some(Rc::new(|error, emitter| {
            emitter.emit("matrix-compaction", json!({"error": error}))
        })),
    })
}
