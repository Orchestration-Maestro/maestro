//! Author calls exercising writable setup imports.
use maestro_extensions_wasm::{
    CommandOptions, ExtensionAPI, ExtensionResult, NewSessionCommandData, NewSessionCommandOptions,
    SessionManager,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};
/// Register a setup command whose body uses the public author facade.
pub fn register(api: &ExtensionAPI) -> ExtensionResult<()> {
    let log = api.clone();
    let kept = Rc::new(RefCell::new(Kept::default()));
    api.register_command(
        "setup",
        CommandOptions {
            description: None,
            get_argument_completions: None,
            handler: Rc::new(move |args, ctx| {
                let log = log.clone();
                let kept = Rc::clone(&kept);
                Box::pin(dispatch(log, kept, args, ctx))
            }),
        },
    )
}
/// Preserve a fallible operation without a success fallback.
fn caught(result: ExtensionResult<Value>) -> Value {
    result.unwrap_or_else(|e| json!({"error":e}))
}
/// Optional authored JSON preserves omission.
fn optional(input: &Value, key: &str) -> Option<Value> {
    input.get(key).cloned()
}
/// Select a writer operation using typed facade inputs.
fn run(writer: &SessionManager, input: &Value) -> ExtensionResult<Value> {
    let args = &input["args"];
    let text = |index: usize| args[index].as_str().unwrap_or("");
    match input["method"].as_str().unwrap_or("isPersisted") {
        "setSessionFile" => writer.set_session_file(text(0)).map(|()| Value::Null),
        "newSession" => writer.new_session(input.get("options").map(|v| maestro_extensions_wasm::NewSessionOptions { id: v["id"].as_str().map(str::to_owned), parent_session: v["parentSession"].as_str().map(str::to_owned) })).map(|v| json!(v)),
        "isPersisted" => writer.is_persisted().map(|v| json!(v)),
        "getChildren" => writer.get_children(text(0)).map(|v| json!(v)),
        "buildSessionContext" => writer.build_session_context().map(|v| json!({"messages":v.messages,"thinkingLevel":v.thinking_level,"model":v.model.map(|m|json!({"provider":m.provider,"modelId":m.model_id}))})),
        "appendMessage" => writer.append_message(authored_message(input)).map(|v| json!(v)),
        "appendThinkingLevelChange" => writer.append_thinking_level_change(text(0)).map(|v| json!(v)),
        "appendModelChange" => writer.append_model_change(text(0),text(1)).map(|v| json!(v)),
        "appendCompaction" => writer.append_compaction(authored_compaction(input), input["fromHook"].as_bool()).map(|v| json!(v)),
        "appendCustomEntry" => writer.append_custom_entry(text(0), optional(input,"data")).map(|v| json!(v)),
        "appendSessionInfo" => writer.append_session_info(text(0)).map(|v| json!(v)),
        "appendCustomMessageEntry" => writer.append_custom_message_entry(serde_json::from_value(input["message"].clone()).unwrap()).map(|v| json!(v)),
        "appendLabelChange" => writer.append_label_change(text(0),input["label"].as_str()).map(|v| json!(v)),
        "branch" => writer.branch(text(0)).map(|()| Value::Null),
        "resetLeaf" => writer.reset_leaf().map(|()| Value::Null),
        "branchWithSummary" => writer.branch_with_summary(input["fromId"].as_str(),text(0),optional(input,"details"),input["fromHook"].as_bool()).map(|v| json!(v)),
        "createBranchedSession" => writer.create_branched_session(text(0)).map(|v| json!(v)),
        "reader" => writer.reader().get_session_name().map(|v| json!(v)),
        "inheritedReader" => writer.get_session_name().map(|v| json!(v)),
        _ => Err("unknown author operation".into()),
    }
}

/// Dispatch an authored invocation without borrowing retained state across host calls.
async fn dispatch(
    log: ExtensionAPI,
    kept: Rc<RefCell<Kept>>,
    args: String,
    ctx: maestro_extensions_wasm::ExtensionCommandContext,
) -> ExtensionResult<()> {
    let input: Value = serde_json::from_str(&args)
        .unwrap_or_else(|_| json!({"method":args,"control":{"value":"written"}}));
    if let Some(control) = input.get("control") {
        log.append_entry("setup-control", Some(control.clone()))?;
    }
    if input["drop"] == true {
        let removed = std::mem::take(&mut *kept.borrow_mut());
        drop(removed);
        return Ok(());
    }
    if input["dropWriter"] == true {
        let removed = kept.borrow_mut().writer.take();
        drop(removed);
        return Ok(());
    }
    if input["retainedReader"] == true {
        let reader = kept.borrow().reader.as_ref().unwrap().clone();
        log.append_entry("setup-answer", Some(json!(reader.get_session_name()?)))?;
        return Ok(());
    }
    if input["retained"] == true {
        let writer = kept.borrow().writer.as_ref().unwrap().clone();
        log.append_entry("setup-answer", Some(caught(run(&writer, &input))))?;
        return Ok(());
    }
    let entered = Rc::new((std::cell::Cell::new(false), RefCell::new(None)));
    let setup = make_setup(&log, &input, &ctx, kept, Rc::clone(&entered));
    let with_session = make_continuation(&log, &input);
    let operation = ctx.new_session(Some(NewSessionCommandOptions {
        data: NewSessionCommandData {
            parent_session: input["parent"].as_str().map(str::to_owned),
        },
        setup,
        with_session,
    }));
    if let Some(phase) = input["dropPhase"].as_str() {
        drop_operation(operation, phase, entered).await;
        log.append_entry("setup-dropped", Some(json!(phase)))?;
        return Ok(());
    }
    let outcome = operation.await;
    log.append_entry(
        "setup-outcome",
        Some(match outcome {
            Ok(v) => json!({"cancelled":v.cancelled}),
            Err(e) => json!({"error":e}),
        }),
    )?;
    Ok(())
}

/// Build a one-shot setup callback, moving future captures separately.
fn make_setup(
    log: &ExtensionAPI,
    input: &Value,
    ctx: &maestro_extensions_wasm::ExtensionCommandContext,
    kept: Rc<RefCell<Kept>>,
    entered: Rc<(std::cell::Cell<bool>, RefCell<Option<std::task::Waker>>)>,
) -> Option<maestro_extensions_wasm::SetupSession> {
    if input["setup"] == false {
        return None;
    }
    let setup_log = log.clone();
    let setup_input = input.clone();
    let held = ctx.clone();
    let mut closure_capture = Capture::new(log, "setup-closure");
    closure_capture.reenter = input["reenter"] == true;
    let future_capture = Capture::new(log, "setup-future");
    (input["setup"] != false).then(|| {
        Box::new(move |writer: SessionManager| {
            entered.0.set(true);
            let wake = entered.1.borrow_mut().take();
            if let Some(wake) = wake {
                wake.wake();
            }
            let result = run(&writer, &setup_input);
            drop(closure_capture);
            if setup_input["retainReader"] == true {
                let old = kept.borrow_mut().reader.replace(writer.reader());
                drop(old);
            }
            if setup_input["retain"] == true {
                let old = kept.borrow_mut().writer.replace(writer);
                drop(old);
            }
            Box::pin(async move {
                let _capture = future_capture;
                finish_setup(setup_log, setup_input, held, result).await
            }) as maestro_extensions_wasm::ExtensionFuture<'static, ()>
        }) as maestro_extensions_wasm::SetupSession
    })
}

/// Build a continuation over the host-supplied operation target.
fn make_continuation(
    log: &ExtensionAPI,
    input: &Value,
) -> Option<maestro_extensions_wasm::WithSession> {
    if input["continuation"] != true {
        return None;
    }
    let continuation_log = log.clone();
    let continuation_input = input.clone();
    let capture = Capture::new(log, "continuation-future");
    (input["continuation"] == true).then(|| {
        Box::new(
            move |ctx: maestro_extensions_wasm::ReplacedSessionContext| {
                Box::pin(async move {
                    let _capture = capture;
                    finish_continuation(&continuation_log, &continuation_input, &ctx)
                }) as maestro_extensions_wasm::ExtensionFuture<'static, ()>
            },
        ) as maestro_extensions_wasm::WithSession
    })
}

/// Await the admitted author work and preserve its error.
async fn finish_setup(
    setup_log: ExtensionAPI,
    setup_input: Value,
    held: maestro_extensions_wasm::ExtensionCommandContext,
    result: ExtensionResult<Value>,
) -> ExtensionResult<()> {
    setup_log.append_entry("setup-answer", Some(caught(result)))?;
    if setup_input["dropPhase"] == "setup" {
        held.wait_for_idle().await?;
    }
    if setup_input["hold"] == true {
        held.wait_for_idle().await?;
    }
    if let Some(error) = setup_input["setupError"].as_str() {
        return Err(error.into());
    }
    Ok(())
}

/// Await the admitted author work and preserve its error.
fn finish_continuation(
    continuation_log: &ExtensionAPI,
    continuation_input: &Value,
    ctx: &maestro_extensions_wasm::ReplacedSessionContext,
) -> ExtensionResult<()> {
    continuation_log.append_entry(
        "setup-continuation",
        Some(if continuation_input["setup"] == false {
            json!(ctx.cwd()?)
        } else {
            json!(ctx.session_manager()?.get_session_name()?)
        }),
    )?;
    if let Some(error) = continuation_input["continuationError"].as_str() {
        return Err(error.into());
    }
    Ok(())
}

/// Capture destruction reported at the public host entry seam.
struct Capture {
    /// Extension used to report the observation.
    api: ExtensionAPI,
    /// Phase whose lifetime is observed.
    phase: &'static str,
    /// Register a callback from this capture's destructor.
    reenter: bool,
}
impl Capture {
    /// A distinct closure or future capture.
    fn new(api: &ExtensionAPI, phase: &'static str) -> Self {
        Self {
            api: api.clone(),
            phase,
            reenter: false,
        }
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.api.append_entry("setup-drop", Some(json!(self.phase)));
        if self.reenter {
            let _ = self.api.on(
                "setup-reentrant",
                Rc::new(|_, _| Box::pin(async { Ok(None) })),
            );
        }
    }
}

/// Drop a never-polled operation or an admitted operation with a pending setup future.
async fn drop_operation(
    mut operation: maestro_extensions_wasm::ExtensionFuture<
        '_,
        maestro_extensions_wasm::SessionChangeResult,
    >,
    phase: &str,
    entered: Rc<(std::cell::Cell<bool>, RefCell<Option<std::task::Waker>>)>,
) {
    if phase == "setup" {
        std::future::poll_fn(|cx| {
            let wake = cx.waker().clone();
            let old = entered.1.borrow_mut().replace(wake);
            drop(old);
            let result = operation.as_mut().poll(cx);
            assert!(result.is_pending());
            if entered.0.get() {
                std::task::Poll::Ready(())
            } else {
                std::task::Poll::Pending
            }
        })
        .await;
    }
    drop(operation);
    let wake = entered.1.borrow_mut().take();
    drop(wake);
}

/// Independently retained writable and read-only capability owners.
#[derive(Default)]
struct Kept {
    /// Writable owner clone.
    writer: Option<SessionManager>,
    /// Separately acquired reader alias.
    reader: Option<maestro_extensions_wasm::ReadonlySessionManager>,
}

/// Mutate a typed number before the real authored encoder runs.
fn authored_message(input: &Value) -> maestro_extensions_wasm::AgentMessage {
    let mut message: maestro_extensions_wasm::AgentMessage =
        serde_json::from_value(input["message"].clone()).unwrap();
    if input["nonfinite"] == true
        && let maestro_extensions_wasm::AgentMessage::Message(message) = &mut message
        && let maestro_extensions_wasm::Message::User(user) = message.as_mut()
    {
        user.timestamp = f64::NAN;
    }
    message
}
/// Supply nonfinite authored tokens without substituting JSON null.
fn authored_compaction(input: &Value) -> maestro_extensions_wasm::CompactionResult {
    let mut value: maestro_extensions_wasm::CompactionResult =
        serde_json::from_value(input["compaction"].clone()).unwrap();
    if input["nonfinite"] == true {
        value.tokens_before = f64::INFINITY;
    }
    value
}
