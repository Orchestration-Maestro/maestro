//! Operation inputs constructed through the public author facade.
use maestro_extensions_wasm::{
    CommandOptions, ExtensionAPI, ExtensionCommandContext, ExtensionFuture, ExtensionResult,
    ForkData, ForkOptions, NavigateTreeOptions, NewSessionCommandData, NewSessionCommandOptions,
    ReplacedSessionContext, SendMessageOptions, SendUserMessageOptions, SwitchSessionOptions,
    WithSession,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};
/// Register an operation-forwarding command with independently retained replacement aliases.
pub fn register(api: &ExtensionAPI) -> ExtensionResult<()> {
    ordinary_callbacks(api)?;
    let log = api.clone();
    let kept = Rc::new(RefCell::new(Kept::default()));
    api.register_command(
        "replacement",
        CommandOptions {
            description: None,
            get_argument_completions: None,
            handler: Rc::new(move |args, ctx| {
                Box::pin(dispatch(log.clone(), Rc::clone(&kept), args, ctx))
            }),
        },
    )
}
/// Read an authored optional value without conflating absence and an empty record.
fn supplied(input: &Value) -> Option<&Value> {
    input["present"]
        .as_bool()
        .unwrap_or(false)
        .then(|| &input["value"])
}
/// Preserve one invocation's result, including an empty error.
fn result(value: ExtensionResult<Value>) -> Value {
    value.map_or_else(|error| json!({"error":error}), |ok| json!({"ok":ok}))
}
/// Dispatch without holding retained state across host calls.
async fn dispatch(
    log: ExtensionAPI,
    kept: Rc<RefCell<Kept>>,
    args: String,
    ctx: ExtensionCommandContext,
) -> ExtensionResult<()> {
    let input: Value = serde_json::from_str(&args).unwrap();
    if let Some(control) = input.get("control") {
        log.append_entry("replacement-control", Some(control.clone()))?;
    }
    if let Some(control) = input.get("setupControl") {
        log.append_entry("setup-control", Some(control.clone()))?;
    }
    if let Some(control) = input.get("readerControl") {
        log.append_entry("reader-control", Some(control.clone()))?;
    }
    if input["drop"] == true {
        let old = std::mem::take(&mut *kept.borrow_mut());
        drop(old);
        return Ok(());
    }
    if input["clone"] == true {
        let alias = kept.borrow().original.as_ref().unwrap().clone();
        let old = kept.borrow_mut().survivor.replace(alias);
        drop(old);
        return Ok(());
    }
    if input["dropOriginal"] == true {
        let old = kept.borrow_mut().original.take();
        drop(old);
        return Ok(());
    }
    if input["retained"] == true {
        return retained_call(&log, &input, &kept).await;
    }
    let entered = Rc::new((std::cell::Cell::new(false), RefCell::new(None)));
    let callback = continuation(&log, &input, Rc::clone(&kept), Rc::clone(&entered));
    let group = input["group"].as_str().unwrap();
    let operation = Box::pin(command(&ctx, group, &input["input"], callback));
    if let Some(phase) = input["dropPhase"].as_str() {
        drop_operation(operation, phase, entered, &ctx).await;
        log.append_entry("replacement-dropped", Some(json!(phase)))?;
        return Ok(());
    }
    let outcome = operation.await;
    log.append_entry("replacement-result", Some(result(outcome)))
}
/// Build a continuation that reads the replacement target before awaiting its own sends.
fn continuation(
    log: &ExtensionAPI,
    input: &Value,
    kept: Rc<RefCell<Kept>>,
    entered: Rc<(std::cell::Cell<bool>, RefCell<Option<std::task::Waker>>)>,
) -> Option<WithSession> {
    let group = input["group"].as_str().unwrap();
    let options = if group == "newSession" {
        &input["input"]["args"][0]
    } else {
        &input["input"]["args"][1]
    };
    let callback = supplied(options).is_some_and(|v| v.get("withSession").is_some());
    if !callback && input["retain"] != true && !group.starts_with("send") {
        return None;
    }
    let log = log.clone();
    let input = input.clone();
    let closure_capture = Capture {
        api: log.clone(),
        phase: "closure",
        reenter: input["reenter"] == true,
    };
    let future_capture = Capture {
        api: log.clone(),
        phase: "future",
        reenter: false,
    };
    Some(Box::new(move |ctx| {
        entered.0.set(true);
        let wake = entered.1.borrow_mut().take();
        if let Some(wake) = wake {
            wake.wake();
        }
        drop(closure_capture);
        if input["retain"] == true {
            let old = if input["target"] == "B" {
                kept.borrow_mut().other.replace(ctx.clone())
            } else {
                kept.borrow_mut().original.replace(ctx.clone())
            };
            drop(old);
        }
        Box::pin(async move {
            let _capture = future_capture;
            finish_continuation(&log, &input, &ctx).await
        })
    }))
}
/// Whether this invocation requests a send rather than a command operation.
fn group_is_send(input: &Value) -> bool {
    input["group"].as_str().unwrap().starts_with("send")
}
/// Construct the independent optional navigation fields.
fn tree(value: &Value) -> NavigateTreeOptions {
    NavigateTreeOptions {
        summarize: value["summarize"].as_bool(),
        replace_instructions: value["replaceInstructions"].as_bool(),
        custom_instructions: value["customInstructions"].as_str().map(str::to_owned),
        label: value["label"].as_str().map(str::to_owned),
    }
}
/// Select exactly the matching public command operation.
async fn command(
    ctx: &ExtensionCommandContext,
    group: &str,
    input: &Value,
    callback: Option<WithSession>,
) -> ExtensionResult<Value> {
    let id = input["args"][0]["value"]
        .as_str()
        .filter(|_| input["args"][0]["present"] != false)
        .unwrap_or("A");
    let options = supplied(&input["args"][1]);
    let outcome = match group {
        "fork" => {
            ctx.fork(
                id,
                options.map(|v| ForkOptions {
                    data: ForkData {
                        position: v
                            .get("position")
                            .map(|p| serde_json::from_value(p.clone()).unwrap()),
                    },
                    with_session: callback,
                }),
            )
            .await?
        }
        "navigateTree" => ctx.navigate_tree(id, options.map(tree)).await?,
        "switchSession" => {
            ctx.switch_session(
                id,
                options.map(|_| SwitchSessionOptions {
                    with_session: callback,
                }),
            )
            .await?
        }
        "reload" => {
            ctx.reload().await?;
            return Ok(Value::Null);
        }
        "newSession" => {
            ctx.new_session(supplied(&input["args"][0]).map(|v| new_options(v, callback)))
                .await?
        }
        "sendMessage" | "sendUserMessage" => {
            ctx.fork(
                "A",
                Some(ForkOptions {
                    data: ForkData::default(),
                    with_session: callback,
                }),
            )
            .await?
        }
        _ => return Err("unknown replacement command".into()),
    };
    Ok(json!({"cancelled":outcome.cancelled}))
}
/// Decode only the test's authored delivery choices.
fn custom_options(input: &Value) -> Option<SendMessageOptions> {
    supplied(input).map(|v| SendMessageOptions {
        trigger_turn: v["triggerTurn"].as_bool(),
        deliver_as: v.get("deliverAs").map(|v| match v.as_str().unwrap() {
            "steer" => maestro_extensions_wasm::MessageDelivery::Steer,
            "followUp" => maestro_extensions_wasm::MessageDelivery::FollowUp,
            "nextTurn" => maestro_extensions_wasm::MessageDelivery::NextTurn,
            _ => panic!("invalid delivery"),
        }),
    })
}
/// Send through the retained capability and read the state that same capability exposes.
async fn send_or_read(
    log: &ExtensionAPI,
    input: &Value,
    ctx: &ReplacedSessionContext,
) -> ExtensionResult<()> {
    let source = &input["input"];
    let outcome = match input["group"].as_str().unwrap() {
        "sendMessage" => {
            let mut payload = source["payload"].clone();
            if let Some(details) = payload.get_mut("details")
                && !details.is_null()
            {
                *details = input
                    .get("opaque")
                    .cloned()
                    .unwrap_or_else(|| json!(details.to_string()));
            }
            ctx.send_message(
                serde_json::from_value(payload).unwrap(),
                custom_options(&source["options"]),
            )
            .await
        }
        "sendUserMessage" => {
            let options = supplied(&source["options"]).map(|v| SendUserMessageOptions {
                deliver_as: v.get("deliverAs").map(|v| match v.as_str().unwrap() {
                    "steer" => maestro_extensions_wasm::UserMessageDelivery::Steer,
                    "followUp" => maestro_extensions_wasm::UserMessageDelivery::FollowUp,
                    _ => panic!("invalid user delivery"),
                }),
            });
            ctx.send_user_message(
                serde_json::from_value(source["payload"].clone()).unwrap(),
                options,
            )
            .await
        }
        "read" => Ok(()),
        "fork" | "navigateTree" | "switchSession" | "reload" => {
            let outcome = command(ctx, input["group"].as_str().unwrap(), source, None).await;
            log.append_entry("replacement-result", Some(result(outcome)))?;
            return Ok(());
        }
        _ => return Err("unknown retained operation".into()),
    };
    log.append_entry(
        "replacement-send-result",
        Some(result(outcome.map(|()| Value::Null))),
    )?;
    log.append_entry(
        "replacement-state",
        Some(json!(ctx.session_manager()?.get_session_name()?)),
    )
}

/// Assemble the setup writer and continuation without mixing their optionality.
fn new_options(value: &Value, callback: Option<WithSession>) -> NewSessionCommandOptions {
    NewSessionCommandOptions {
        data: NewSessionCommandData {
            parent_session: value["parentSession"].as_str().map(str::to_owned),
        },
        setup: value.get("setup").map(|_| {
            Box::new(|writer: maestro_extensions_wasm::SessionManager| {
                Box::pin(async move {
                    writer.append_session_info("setup-marker")?;
                    Ok(())
                }) as ExtensionFuture<'static, ()>
            }) as maestro_extensions_wasm::SetupSession
        }),
        with_session: callback,
    }
}

/// Exercise the actual ordinary context types accepted by shortcuts and tools.
fn ordinary_callbacks(api: &ExtensionAPI) -> ExtensionResult<()> {
    let log = api.clone();
    api.register_shortcut("replacement-ordinary", maestro_extensions_wasm::ShortcutOptions {
        description: None,
        handler: Rc::new(move |ctx: maestro_extensions_wasm::ExtensionContext| {
            let log = log.clone();
            Box::pin(async move { log.append_entry("ordinary-shortcut", Some(json!({"cwd":ctx.cwd()?,"name":ctx.session_manager()?.get_session_name()?}))) })
        }),
    })?;
    let log = api.clone();
    api.register_tool(maestro_extensions_wasm::ToolDefinition {
        render_call: None,
        render_result: None,
        metadata: serde_json::from_value(json!({"name":"replacement-ordinary","label":"Ordinary","description":"Reads its supplied context","parameters":"{}"})).unwrap(),
        prepare_arguments: None,
        execute: Rc::new(move |_, _, _, _, ctx: maestro_extensions_wasm::ExtensionContext| {
            let log = log.clone();
            Box::pin(async move {
                log.append_entry("ordinary-tool", Some(json!({"cwd":ctx.cwd()?,"name":ctx.session_manager()?.get_session_name()?})))?;
                Ok(serde_json::from_value(json!({"content":[{"type":"text","text":"ordinary answer"}],"details":"null"})).unwrap())
            })
        }),
    })
}

/// Independently retained original, surviving alias and distinguishable second target.
#[derive(Default)]
struct Kept {
    /// Original alias from the callback.
    original: Option<ReplacedSessionContext>,
    /// Clone that outlives removal of the original slot.
    survivor: Option<ReplacedSessionContext>,
    /// Separate target used to reject global-state routing.
    other: Option<ReplacedSessionContext>,
}
/// Observable capture teardown, including a reentrant registration.
struct Capture {
    /// Public host seam used by the destructor.
    api: ExtensionAPI,
    /// Closure versus returned future.
    phase: &'static str,
    /// Register an event while releasing an unused callback.
    reenter: bool,
}
impl Drop for Capture {
    fn drop(&mut self) {
        self.api
            .append_entry("replacement-drop", Some(json!(self.phase)))
            .unwrap();
        if self.reenter {
            self.api
                .on(
                    "replacement-reentrant",
                    Rc::new(|_, _| Box::pin(async { Ok(None) })),
                )
                .unwrap();
        }
    }
}
/// Drop an unpolled operation or wait for the independently admitted callback before dropping.
async fn drop_operation<F: std::future::Future>(
    mut operation: std::pin::Pin<Box<F>>,
    phase: &str,
    entered: Rc<(std::cell::Cell<bool>, RefCell<Option<std::task::Waker>>)>,
    ctx: &ExtensionCommandContext,
) {
    if phase == "callback" {
        std::future::poll_fn(|cx| {
            let wake = cx.waker().clone();
            let old = entered.1.borrow_mut().replace(wake);
            drop(old);
            assert!(operation.as_mut().poll(cx).is_pending());
            if entered.0.get() {
                std::task::Poll::Ready(())
            } else {
                std::task::Poll::Pending
            }
        })
        .await;
    }
    if phase == "host" {
        let mut notification = ctx.wait_for_idle();
        std::future::poll_fn(|cx| {
            assert!(operation.as_mut().poll(cx).is_pending());
            notification.as_mut().poll(cx)
        })
        .await
        .unwrap();
    }
    drop(operation);
    let wake = entered.1.borrow_mut().take();
    drop(wake);
}

#[path = "replacement_constructor.rs"]
mod constructor;

/// Select retained ports outside their container borrow before calling the public facade.
async fn retained_call(
    log: &ExtensionAPI,
    input: &Value,
    kept: &RefCell<Kept>,
) -> ExtensionResult<()> {
    let held = kept.borrow().select(input["target"] == "B").clone();
    if input["constructor"] != true {
        return send_or_read(log, input, &held).await;
    }
    let sibling = kept.borrow().select(input["target"] != "B").clone();
    let original = constructor::construct(log.clone(), held, sibling);
    let survivor = original.clone();
    drop(original);
    log.append_entry("constructor-command", Some(json!(survivor.cwd()?)))?;
    send_or_read(log, input, &survivor).await
}
impl Kept {
    /// Select an independently retained resource without invoking host code.
    fn select(&self, other: bool) -> &ReplacedSessionContext {
        if other {
            self.other.as_ref()
        } else {
            self.original.as_ref().or(self.survivor.as_ref())
        }
        .unwrap()
    }
}

/// Await one admitted continuation's work after its immediate captures have been released.
async fn finish_continuation(
    log: &ExtensionAPI,
    input: &Value,
    ctx: &ReplacedSessionContext,
) -> ExtensionResult<()> {
    log.append_entry(
        "replacement-target",
        Some(json!({"cwd":ctx.cwd()?,"closure":input["closure"]})),
    )?;
    if input["closure"].is_string() {
        log.append_entry(
            "replacement-reader",
            Some(json!(ctx.session_manager()?.get_session_name()?)),
        )?;
    }
    if input["dropPhase"] == "callback" || input["holdCallback"] == true {
        ctx.wait_for_idle().await?;
    }
    if group_is_send(input) {
        send_or_read(log, input, ctx).await?;
    }
    if let Some(error) = input["authorError"].as_str() {
        return Err(error.into());
    }
    log.append_entry("replacement-continued", None)?;
    Ok(())
}
