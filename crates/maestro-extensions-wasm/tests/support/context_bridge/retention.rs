//! Context reads, replacement operands and continued messages the originating frame keeps.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    CustomMessageInput, ExtensionCommandContext, ExtensionContext, ExtensionResult, ForkOptions,
    ForkPosition, JsonText, MessageDelivery, NewSessionCommandData, NewSessionCommandOptions,
    Presence, ReplacedSessionContext, SendMessageOptions, SendUserMessageOptions, SetupSession,
    SwitchSessionOptions, TextContent, UserBlock, UserContent, UserMessageDelivery, WithSession,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

thread_local! {
    /// Ordered observations the supplied and authored callbacks leave for the frame.
    static TRACE: RefCell<Vec<String>> = RefCell::default();
    /// The continuation's own context, retained after the initiating operation returns.
    static REPLACED: RefCell<Option<ReplacedSessionContext>> = RefCell::default();
}

/// Runs one retained-context request from the actual authored command frame.
pub(super) async fn command(
    context: &ExtensionCommandContext,
    request: &str,
) -> ExtensionResult<()> {
    let (member, detail) = request.split_once(' ').unwrap_or((request, "none"));
    let report = match member {
        "lazy" => lazy(context).await?,
        "replaced-messages" => replaced_messages(context).await?,
        "new-session" => new_session(context, detail).await?,
        "fork" => fork(context, detail).await?,
        "switch" => switch(context, detail).await?,
        "errors" => errors(context, detail).await?,
        _ => return Err(format!("unknown retention request: {member}")),
    };
    Err(report.to_string())
}

/// Notes one ordered observation so later assertions see the real callback order.
fn note(line: String) {
    TRACE.with(|trace| trace.borrow_mut().push(line));
}

/// Returns the observations of the current command and starts a new order.
fn drained() -> Vec<String> {
    TRACE.with(|trace| std::mem::take(&mut *trace.borrow_mut()))
}

/// Reports a member's own value or its rejection without normalizing either one.
fn observed(name: &str, result: ExtensionResult<Value>) -> Value {
    match result {
        Ok(value) => json!({ "name": name, "ok": value }),
        Err(error) => json!({ "name": name, "err": error }),
    }
}

/// Reads every guarded base member through the supplied context.
fn members(context: &ExtensionContext) -> Vec<Value> {
    vec![
        observed("ui", context.ui().map(|_| Value::Null)),
        observed("hasUI", context.has_ui().map(Value::Bool)),
        observed("cwd", context.cwd().map(Value::String)),
        observed("sessionManager", identity(context)),
        observed("modelRegistry", catalog_ids(context)),
        observed("model", model_name(context)),
        observed("isIdle", context.is_idle().map(Value::Bool)),
        observed("signal", signal_state(context)),
        observed(
            "hasPendingMessages",
            context.has_pending_messages().map(Value::Bool),
        ),
        observed("getContextUsage", usage_tokens(context)),
        observed(
            "getSystemPrompt",
            context.get_system_prompt().map(Value::String),
        ),
    ]
}

/// Reports the describing session's own directory and identity.
fn identity(context: &ExtensionContext) -> ExtensionResult<Value> {
    let history = context.session_manager()?;
    Ok(json!({ "cwd": history.get_cwd()?, "id": history.get_session_id()? }))
}

/// Reports the identifiers of every supplied catalog entry.
fn catalog_ids(context: &ExtensionContext) -> ExtensionResult<Value> {
    let catalog = context.model_registry()?;
    Ok(json!(
        catalog
            .get_all()?
            .into_iter()
            .map(|model| model.id)
            .collect::<Vec<_>>()
    ))
}

/// Reports the name of the model the context captured when it was created.
fn model_name(context: &ExtensionContext) -> ExtensionResult<Value> {
    Ok(json!(context.model()?.map(|model| model.name)))
}

/// Reports whether an acquired cancellation flag is set.
fn signal_state(context: &ExtensionContext) -> ExtensionResult<Value> {
    Ok(json!(context.signal()?.map(|signal| signal.aborted())))
}

/// Reports the supplied token estimate without its unrelated window fields.
fn usage_tokens(context: &ExtensionContext) -> ExtensionResult<Value> {
    Ok(json!(
        context.get_context_usage()?.and_then(|usage| usage.tokens)
    ))
}

/// Runs the five session-replacing operations that do not wait for the agent.
async fn operations(context: &ExtensionCommandContext) -> Vec<Value> {
    let change = |result: ExtensionResult<maestro_extensions_wasm::SessionChangeResult>| {
        result.map(|result| json!({ "cancelled": result.cancelled }))
    };
    vec![
        observed("newSession", change(context.new_session(None).await)),
        observed("fork", change(context.fork("entry-matrix", None).await)),
        observed(
            "navigateTree",
            change(context.navigate_tree("target-matrix", None).await),
        ),
        observed(
            "switchSession",
            change(context.switch_session("/sessions/matrix.jsonl", None).await),
        ),
        observed("reload", context.reload().await.map(|()| Value::Null)),
    ]
}

/// Reads every member and operation once, before and after each independent rebinding.
async fn phase(context: &ExtensionCommandContext) -> Value {
    json!({ "members": members(context), "operations": operations(context).await })
}

/// Reports each suspension separately so supplied bindings can change between them.
async fn lazy(context: &ExtensionCommandContext) -> ExtensionResult<Value> {
    let mut phases = vec![phase(context).await];
    for _ in 0..2 {
        context.wait_for_idle().await?;
        phases.push(phase(context).await);
    }
    context.wait_for_idle().await?;
    let wait = match context.wait_for_idle().await {
        Ok(()) => Value::Null,
        Err(error) => Value::String(error),
    };
    let mut last = phase(context).await;
    last["wait"] = wait;
    phases.push(last);
    Ok(Value::Array(phases))
}

/// Describes the plain record a presence case sends.
fn presence_data(combo: &str) -> NewSessionCommandData {
    NewSessionCommandData {
        parent_session: matches!(combo, "parent" | "full")
            .then(|| "  /sessions/\u{feff}padded parent.jsonl ".to_owned()),
    }
}

/// Builds only the callbacks the selected presence case carries.
fn presence_options(combo: &str) -> Option<NewSessionCommandOptions> {
    if combo == "none" {
        return None;
    }
    Some(NewSessionCommandOptions {
        setup: matches!(combo, "setup" | "both" | "full").then(setup_writer),
        data: presence_data(combo),
        with_session: matches!(combo, "with" | "both" | "full").then(|| continuation("presence")),
    })
}

/// Writes through the real writable session supplied to a one-shot setup.
fn setup_writer() -> SetupSession {
    Box::new(move |session| {
        Box::pin(async move {
            note("setup".into());
            session.append_session_info("literal setup name")?;
            Ok(())
        })
    })
}

/// Records the fresh context an operation's continuation receives.
fn continuation(label: &'static str) -> WithSession {
    Box::new(move |replaced| {
        Box::pin(async move {
            note(format!("continuation:{label}:{}", replaced.cwd()?));
            Ok(())
        })
    })
}

/// Reports one operation's result and the order its callbacks ran in.
fn completed(result: ExtensionResult<maestro_extensions_wasm::SessionChangeResult>) -> Value {
    match result {
        Ok(result) => json!({ "cancelled": result.cancelled, "trace": drained() }),
        Err(error) => json!({ "err": error, "trace": drained() }),
    }
}

/// Forwards each presence combination of the new-session options.
async fn new_session(context: &ExtensionCommandContext, combo: &str) -> ExtensionResult<Value> {
    let result = context.new_session(presence_options(combo)).await;
    Ok(completed(result))
}

/// Forwards the exact fork position and its independently optional continuation.
async fn fork(context: &ExtensionCommandContext, detail: &str) -> ExtensionResult<Value> {
    let (position, with) = match detail {
        "absent" => (None, false),
        "empty" => (None, true),
        "before" => (Some(ForkPosition::Before), false),
        "at" => (Some(ForkPosition::At), true),
        _ => return Err(format!("unknown fork case: {detail}")),
    };
    let entry = if detail == "at" {
        "  e\u{feff}\u{85} "
    } else {
        "entry-fork"
    };
    let options = Some(ForkOptions {
        data: maestro_extensions_wasm::ForkData { position },
        with_session: with.then(|| continuation("fork")),
    });
    let result = context.fork(entry, options).await;
    Ok(completed(result))
}

/// Forwards an authored session path without trimming or resolving it.
async fn switch(context: &ExtensionCommandContext, detail: &str) -> ExtensionResult<Value> {
    let (path, options) = match detail {
        "absent" => ("  /sessions/ padded name.jsonl  ", None),
        "empty" => (
            "/sessions/\u{feff}bom.jsonl",
            Some(SwitchSessionOptions::default()),
        ),
        "continuation" => (
            "/sessions/\u{85}next-line.jsonl",
            Some(SwitchSessionOptions {
                with_session: Some(continuation("switch")),
            }),
        ),
        _ => return Err(format!("unknown switch case: {detail}")),
    };
    let result = context.switch_session(path, options).await;
    let mut report = completed(result);
    report["path"] = json!(path);
    Ok(report)
}

/// Reports genuine setup and continuation failures at the originating frame.
async fn errors(context: &ExtensionCommandContext, mode: &str) -> ExtensionResult<Value> {
    let options = NewSessionCommandOptions {
        setup: Some(failing_setup(mode)),
        data: NewSessionCommandData {
            parent_session: None,
        },
        with_session: Some(failing_continuation(mode)),
    };
    let result = context.new_session(Some(options)).await;
    Ok(completed(result))
}

/// Fails the setup of the selected mode before its continuation could run.
fn failing_setup(mode: &str) -> SetupSession {
    let fails = mode == "setup";
    Box::new(move |session| {
        Box::pin(async move {
            note("setup".into());
            if fails {
                return Err("literal setup failure".into());
            }
            session.append_session_info("literal setup effect")?;
            Ok(())
        })
    })
}

/// Fails the continuation after its own completed effect.
fn failing_continuation(mode: &str) -> WithSession {
    let fails = mode == "continuation";
    Box::new(move |replaced| {
        Box::pin(async move {
            note(format!("continuation:{}", replaced.cwd()?));
            if fails {
                return Err("literal continuation failure".into());
            }
            Ok(())
        })
    })
}

/// Keeps the describing session's messages reachable after the context is invalidated.
async fn replaced_messages(context: &ExtensionCommandContext) -> ExtensionResult<Value> {
    let retained = Rc::new(RefCell::new(None));
    let slot = retained.clone();
    let with_session: WithSession = Box::new(move |replaced| {
        let slot = slot.clone();
        Box::pin(async move {
            note(format!("continuation:{}", replaced.cwd()?));
            sends(&replaced, "before").await;
            *slot.borrow_mut() = Some(replaced.clone());
            Ok(())
        })
    });
    let result = context
        .new_session(Some(NewSessionCommandOptions {
            setup: None,
            data: NewSessionCommandData {
                parent_session: Some("/sessions/parent-messages.jsonl".into()),
            },
            with_session: Some(with_session),
        }))
        .await;
    let resolved = completed(result);
    let retained = retained.borrow_mut().take();
    context.wait_for_idle().await?;
    let Some(retained) = retained else {
        return Err("replacement context was not retained".into());
    };
    let guarded = match retained.cwd() {
        Ok(cwd) => json!({ "ok": cwd }),
        Err(error) => json!({ "err": error }),
    };
    let after = sends(&retained, "after").await;
    Ok(json!({
        "resolved": resolved,
        "guarded": guarded,
        "after": after,
        "retained": !retained.send_message(absent_message(), None).await.is_err(),
    }))
}

/// Sends one message per accepted details domain and both message operations.
async fn sends(context: &ReplacedSessionContext, phase: &str) -> Vec<Value> {
    let mut values = Vec::new();
    for (index, details) in detail_domains().into_iter().enumerate() {
        let message = CustomMessageInput {
            custom_type: format!("{phase}-{index}"),
            content: UserContent::Text(format!("{phase} text {index}")),
            display: index % 2 == 0,
            details,
        };
        let options = delivery(index).map(|deliver_as| SendMessageOptions {
            trigger_turn: Some(index % 2 == 0),
            deliver_as: Some(deliver_as),
        });
        values.push(observed(
            &format!("message{index}"),
            context
                .send_message(message, options)
                .await
                .map(|()| Value::Null),
        ));
    }
    values.extend(user_messages(context, phase).await);
    values
}

/// Supplies every details domain the source accepts, including both null and absence.
fn detail_domains() -> Vec<Presence<JsonText>> {
    vec![
        Presence::Missing,
        Presence::Present(JsonText::from(String::from("null"))),
        Presence::Present(JsonText::from(json!(" literal details ").to_string())),
        Presence::Present(JsonText::from(String::from("-0.0"))),
        Presence::Present(JsonText::from(String::from("true"))),
        Presence::Present(JsonText::from(
            json!({ "z": 1, "a": [2.5, null] }).to_string(),
        )),
        Presence::Present(JsonText::from(json!([1, "two", null]).to_string())),
    ]
}

/// Selects each accepted custom-message delivery placement in turn.
fn delivery(index: usize) -> Option<MessageDelivery> {
    match index % 4 {
        1 => Some(MessageDelivery::Steer),
        2 => Some(MessageDelivery::FollowUp),
        3 => Some(MessageDelivery::NextTurn),
        _ => None,
    }
}

/// Sends authored text and ordered blocks through the user-message operation.
async fn user_messages(context: &ReplacedSessionContext, phase: &str) -> Vec<Value> {
    let blocks = vec![
        UserBlock::Text(TextContent::new(format!("{phase} first"))),
        UserBlock::Text(TextContent::new(String::new())),
        UserBlock::Text(TextContent::new(format!("{phase} last"))),
    ];
    let user = SendUserMessageOptions {
        deliver_as: Some(UserMessageDelivery::FollowUp),
    };
    vec![
        observed(
            "user-text",
            context
                .send_user_message(
                    UserContent::Text(format!("{phase} user text")),
                    Some(SendUserMessageOptions::default()),
                )
                .await
                .map(|()| Value::Null),
        ),
        observed(
            "user-blocks",
            context
                .send_user_message(UserContent::Blocks(blocks), Some(user))
                .await
                .map(|()| Value::Null),
        ),
    ]
}

/// An absent-details message used to observe whether the retained context still sends.
fn absent_message() -> CustomMessageInput {
    CustomMessageInput {
        custom_type: "retained-probe".into(),
        content: UserContent::Text("retained probe".into()),
        display: false,
        details: Presence::Missing,
    }
}
