//! Author operations for live context observations and retained capabilities.
use maestro_extensions_wasm::{
    AbortSignal, CommandOptions, ContextUsage, ExtensionAPI, ExtensionCommandContext,
    ExtensionContext, ExtensionEvent, ExtensionEventResult, ExtensionResult, ExtensionUIContext,
    InputEventResult, InputTransform, Model, ModelRegistry, NewSessionCommandData,
    NewSessionCommandOptions, Presence, ResolvedRequestAuth,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::cell::RefCell;
use std::rc::Rc;
/// One public operation with an acknowledged host change.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    /// Host mutation completed before observing the facade.
    control: Option<Value>,
    /// Selected operation.
    action: String,
    /// Literal getter or catalog route.
    #[serde(default)]
    method: String,
    /// Literal query operands.
    #[serde(default)]
    args: Vec<Value>,
    /// Query the retained resource.
    #[serde(default)]
    retained: bool,
    /// Supply a nonfinite authored model cost.
    #[serde(default)]
    nonfinite: bool,
}
/// Aliases deliberately retained after callback return.
#[derive(Default)]
struct Kept {
    /// Captured ordinary context.
    context: Option<ExtensionContext>,
    /// Independently retained catalogs.
    registries: Vec<ModelRegistry>,
    /// Independently retained UI owners.
    uis: Vec<ExtensionUIContext>,
    /// Independently retained cancellation flags.
    signals: Vec<AbortSignal>,
}
/// Register the same public observer on ordinary and command contexts.
pub fn register(api: &ExtensionAPI) -> ExtensionResult<()> {
    let kept = Rc::new(RefCell::new(Kept::default()));
    let event_kept = Rc::clone(&kept);
    let log = api.clone();
    api.on(
        "catalog",
        Rc::new(move |event, ctx| {
            let kept = Rc::clone(&event_kept);
            let log = log.clone();
            Box::pin(async move {
                let ExtensionEvent::Input(input) = event else {
                    return Err("expected input".into());
                };
                let request = prepare(&log, &input.text)?;
                let answer = caught(run(&kept, request, &ctx).await);
                Ok(Some(ExtensionEventResult::Input(
                    InputEventResult::Transform(InputTransform {
                        text: answer.to_string(),
                        images: Presence::Missing,
                    }),
                )))
            })
        }),
    )?;
    register_command(api, kept)?;
    register_compaction(api)
}
/// Register command and replacement-context observations.
fn register_command(api: &ExtensionAPI, kept: Rc<RefCell<Kept>>) -> ExtensionResult<()> {
    let log = api.clone();
    api.register_command(
        "catalog",
        CommandOptions {
            get_argument_completions: None,
            description: None,
            handler: Rc::new(move |args, ctx| {
                let kept = Rc::clone(&kept);
                let log = log.clone();
                Box::pin(async move {
                    let request = prepare(&log, &args)?;
                    if request.action == "replacement" {
                        return replacement(log, kept, request, ctx).await;
                    }
                    let answer = caught(run(&kept, request, &ctx).await);
                    log.append_entry("catalog-answer", Some(answer))
                })
            }),
        },
    )
}
/// Invoke the inherited ordinary view or catalog from an actual replacement continuation.
async fn replacement(
    log: ExtensionAPI,
    kept: Rc<RefCell<Kept>>,
    mut request: Request,
    ctx: ExtensionCommandContext,
) -> ExtensionResult<()> {
    request.action = if matches!(request.method.as_str(), "getAll" | "getAvailable" | "find") {
        "catalog"
    } else {
        "context"
    }
    .into();
    ctx.new_session(Some(NewSessionCommandOptions {
        setup: None,
        data: NewSessionCommandData {
            parent_session: None,
        },
        with_session: Some(Box::new(move |ctx| {
            Box::pin(async move {
                let answer = caught(run(&kept, request, &ctx).await);
                log.append_entry("catalog-answer", Some(answer))
            })
        })),
    }))
    .await?;
    Ok(())
}
/// Complete the host control before reading the selected capability.
fn prepare(api: &ExtensionAPI, text: &str) -> ExtensionResult<Request> {
    let mut request: Request = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if let Some(control) = request.control.take() {
        api.append_entry("catalog-control", Some(control))?;
    }
    Ok(request)
}
/// Return caught host failures without failing the callback.
fn caught(answer: ExtensionResult<Value>) -> Value {
    match answer {
        Ok(value) => json!({"ok":value}),
        Err(error) => json!({"error":error}),
    }
}
/// Dispatch one operation, releasing borrows before invoking the facade.
async fn run(
    kept: &RefCell<Kept>,
    request: Request,
    ctx: &ExtensionContext,
) -> ExtensionResult<Value> {
    match request.action.as_str() {
        "control" => Ok(json!("ack")),
        "context" => context_query(ctx, &request.method),
        "keep-context" => {
            let old = kept.borrow_mut().context.replace(ctx.clone());
            drop(old);
            Ok(json!("kept"))
        }
        "old-context" => {
            let ctx = kept.borrow().context.clone().ok_or("missing context")?;
            context_query(&ctx, &request.method)
        }
        "signal" | "ui" => retained(kept, &request, ctx),
        "catalog" => catalog(kept, &request, ctx).await,
        "drop" => {
            let old = std::mem::take(&mut *kept.borrow_mut());
            drop(old);
            Ok(json!("dropped"))
        }
        _ => Err("unknown operation".into()),
    }
}
/// Read the host state through each ordinary context method.
fn context_query(ctx: &ExtensionContext, method: &str) -> ExtensionResult<Value> {
    match method {
        "ui" => {
            let _ui = ctx.ui()?;
            Ok(json!(true))
        }
        "hasUI" => Ok(json!(ctx.has_ui()?)),
        "modelRegistry" => {
            let _catalog = ctx.model_registry()?;
            Ok(json!(true))
        }
        "model" => encoded(ctx.model()?),
        "isIdle" => Ok(json!(ctx.is_idle()?)),
        "signal" => Ok(json!(ctx.signal()?.map(|s| s.aborted()))),
        "abort" => {
            ctx.abort()?;
            Ok(Value::Null)
        }
        "hasPendingMessages" => Ok(json!(ctx.has_pending_messages()?)),
        "shutdown" => {
            ctx.shutdown()?;
            Ok(Value::Null)
        }
        "getContextUsage" => Ok(ctx.get_context_usage()?.map_or(Value::Null, usage)),
        "getSystemPrompt" => Ok(json!(ctx.get_system_prompt()?)),
        _ => Err("unknown context query".into()),
    }
}
/// Preserve numeric zero signs in a public result alongside usage fields.
fn usage(u: ContextUsage) -> Value {
    json!({"tokens":u.tokens,"contextWindow":u.context_window,"percent":u.percent,"signs":[u.tokens.map(f64::is_sign_negative),u.context_window.is_sign_negative(),u.percent.map(f64::is_sign_negative)]})
}
/// Retain, clone, observe or drop cancellation and opaque UI owners.
fn retained(
    kept: &RefCell<Kept>,
    request: &Request,
    ctx: &ExtensionContext,
) -> ExtensionResult<Value> {
    if request.action == "signal" {
        match request.method.as_str() {
            "retain" => {
                let signal = ctx.signal()?.ok_or("missing signal")?;
                kept.borrow_mut().signals.push(signal);
            }
            "alias" => {
                let signal = kept
                    .borrow()
                    .signals
                    .first()
                    .cloned()
                    .ok_or("missing signal")?;
                kept.borrow_mut().signals.push(signal);
            }
            "read" => {
                let signal = kept
                    .borrow()
                    .signals
                    .first()
                    .cloned()
                    .ok_or("missing signal")?;
                return Ok(json!(signal.aborted()));
            }
            "pop" => {
                let old = kept.borrow_mut().signals.pop();
                drop(old);
            }
            _ => return Err("unknown signal operation".into()),
        }
    } else if let Some(answer) = retained_ui(kept, request, ctx)? {
        return Ok(answer);
    }
    Ok(json!("done"))
}
/// Retain, clone, drop or call through an opaque UI owner, by its retained index, returning
/// what a read request answered. A drop names its index in the arguments and takes the last
/// owner without one.
fn retained_ui(
    kept: &RefCell<Kept>,
    request: &Request,
    ctx: &ExtensionContext,
) -> ExtensionResult<Option<Value>> {
    match request.method.as_str() {
        "retain" => kept.borrow_mut().uis.push(ctx.ui()?),
        "alias" => {
            let ui = kept.borrow().uis.first().cloned().ok_or("missing UI")?;
            kept.borrow_mut().uis.push(ui);
        }
        "pop" => {
            let count = kept.borrow().uis.len();
            let index = request.args.first().and_then(Value::as_u64).map_or_else(
                || count.saturating_sub(1),
                |index| usize::try_from(index).unwrap_or(usize::MAX),
            );
            let old = (index < count).then(|| kept.borrow_mut().uis.remove(index));
            drop(old);
        }
        "call" => {
            let index = request
                .args
                .get(1)
                .and_then(Value::as_u64)
                .map_or(0, |index| usize::try_from(index).unwrap_or(usize::MAX));
            let ui = kept.borrow().uis.get(index).cloned().ok_or("missing UI")?;
            let call = serde_json::from_value(request.args.first().cloned().ok_or("missing call")?)
                .map_err(|error| error.to_string())?;
            return super::ui_author::perform(&ui, &call);
        }
        _ => return Err("unknown UI operation".into()),
    }
    Ok(None)
}
/// Query or manage an independently retained catalog.
async fn catalog(
    kept: &RefCell<Kept>,
    request: &Request,
    ctx: &ExtensionContext,
) -> ExtensionResult<Value> {
    if request.method == "pop" {
        let old = kept.borrow_mut().registries.pop();
        drop(old);
        return Ok(json!("done"));
    }
    let registry = if request.retained {
        kept.borrow()
            .registries
            .first()
            .cloned()
            .ok_or("missing catalog")?
    } else {
        ctx.model_registry()?
    };
    match request.method.as_str() {
        "retain" | "alias" => {
            kept.borrow_mut().registries.push(registry);
            Ok(json!("done"))
        }
        "getAll" => encoded(registry.get_all()?),
        "getAvailable" => encoded(registry.get_available()?),
        "find" => encoded(
            registry.find(
                request
                    .args
                    .first()
                    .and_then(Value::as_str)
                    .ok_or("missing provider")?,
                request
                    .args
                    .get(1)
                    .and_then(Value::as_str)
                    .ok_or("missing model id")?,
            )?,
        ),
        "auth" => {
            let mut model: Model =
                serde_json::from_value(request.args[0].clone()).map_err(|e| e.to_string())?;
            if request.nonfinite {
                model.cost.input = f64::INFINITY;
            }
            match registry.get_api_key_and_headers(model).await? {
                ResolvedRequestAuth::Ok {
                    api_key, headers, ..
                } => Ok(
                    json!({"apiKey":api_key,"headers":headers.map(|h|h.into_iter().collect::<Vec<_>>())}),
                ),
                ResolvedRequestAuth::Error { error } => Ok(json!({"error":error})),
            }
        }
        _ => Err("unknown catalog query".into()),
    }
}
/// Serialize the actual shared typed value, not a projection.
fn encoded(value: impl serde::Serialize) -> ExtensionResult<Value> {
    serde_json::to_value(value).map_err(|e| e.to_string())
}
/// Execute the example handler, making its details depend on live capabilities.
fn register_compaction(api: &ExtensionAPI) -> ExtensionResult<()> {
    api.on(
        "catalog-compaction",
        Rc::new(move |event, ctx| {
            Box::pin(async move {
                let reader = ctx.session_manager()?;
                let catalog = ctx.model_registry()?;
                let leaf = reader.get_leaf_id()?;
                let names = catalog
                    .get_all()?
                    .into_iter()
                    .map(|m| m.name)
                    .collect::<Vec<_>>();
                let ExtensionEvent::Session(maestro_extensions_wasm::SessionEvent::BeforeCompact(
                    compact,
                )) = event
                else {
                    return Err("expected compaction".into());
                };
                let branch = encoded(&compact.branch_entries)?;
                let mut result = super::summarize(event)?;
                if let ExtensionEventResult::SessionBeforeCompact(result) = &mut result
                    && let Presence::Present(compaction) = &mut result.compaction
                {
                    compaction.details = Presence::Present(
                        json!({"leaf":leaf,"models":names,"branchEntries":branch})
                            .to_string()
                            .into(),
                    );
                }
                Ok(Some(result))
            })
        }),
    )
}
