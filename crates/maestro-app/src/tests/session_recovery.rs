//! Compacting a conversation and retrying a failed response, compared with the recorded
//! recovery cases: the announced events, the rebuilt history and the stored entries.
#![cfg(not(target_arch = "wasm32"))]
use super::session_harness::{
    Auth, Harness, HarnessOptions, answer, as_recorded, assistant, case, harness, load_history,
    message_text, on_paused_task_set, recorded_number, script_responses, start_and_abandon,
    summary_provider, summary_result, usage, user,
};
use maestro_agent::AgentMessage;
use maestro_app::agent_session::CompactionReason;
use maestro_app::{AgentSession, AgentSessionEvent};
use maestro_extensions::{
    ExtensionAPI, ExtensionFactory, SessionBeforeCompactEvent, SessionBeforeCompactResult,
    SessionBeforeTreeEvent, SessionBeforeTreeResult, SessionCompactEvent, SessionTreeEvent,
};
use maestro_models::{
    AssistantMessage, DiagnosticErrorInfo, FauxAssistantMessageOptions, Local, StopReason,
    faux_assistant_message,
};
use maestro_session::CompactionResult;
use maestro_session::Presence;
use maestro_session::SessionManager;
use maestro_session::messages::CustomAgentMessages;
use maestro_session::session_manager::{CompactionOptions, ReadonlySessionManager};
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    sync::{Arc, RwLock},
};
use tokio::sync::Notify;

/// The context window every recorded case ran with.
const WINDOW: f64 = 200_000.0;

/// The compaction preferences every recorded case ran with: the newest message only is kept.
fn keep_recent(settings: Value) -> Value {
    let mut settings = settings;
    settings["compaction"]["keepRecentTokens"] = json!(1);
    settings
}

/// A session over the scripted provider, holding the recorded starting conversation.
async fn seeded(settings: Value, factories: Vec<ExtensionFactory<Local>>) -> Harness {
    seeded_window(WINDOW, settings, factories).await
}

/// A seeded session whose model reports the supplied context window.
async fn seeded_window(
    window: f64,
    settings: Value,
    factories: Vec<ExtensionFactory<Local>>,
) -> Harness {
    let h = harness(HarnessOptions {
        settings: keep_recent(settings),
        factories,
        context_window: Some(window),
        ..HarnessOptions::default()
    })
    .await;
    seed_history(&h);
    h
}

/// A session over the scripted provider with credentials left unconfigured.
async fn unauthenticated(factories: Vec<ExtensionFactory<Local>>) -> Harness {
    harness(HarnessOptions {
        settings: keep_recent(json!({})),
        factories,
        auth: Auth::None,
        context_window: Some(WINDOW),
        ..HarnessOptions::default()
    })
    .await
}

/// Appends the recorded starting conversation and gives the agent the same history.
fn seed_history(h: &Harness) {
    let records = [
        user(&json!([{"type": "text", "text": "one"}]), 100),
        assistant("", 200, &answer_extra(&StopReason::Stop, None, 10)),
        user(&json!([{"type": "text", "text": "two"}]), 300),
    ];
    let manager = h.session.session_manager();
    let mut manager = manager.borrow_mut();
    for record in records {
        drop(manager.append_message(record).unwrap());
    }
    drop(manager);
    load_history(h);
}

/// The extra fields of a recorded scripted answer: its outcome, failure and usage.
fn answer_extra(stop: &StopReason, error: Option<&str>, total: u64) -> Value {
    let mut extra = json!({
        "stopReason": stop,
        "usage": usage([total, 0, 0, 0], 0.0, Some(total)),
    });
    if let Some(error) = error {
        extra["errorMessage"] = json!(error);
    }
    extra
}

/// The response record every recorded case passed to the session.
fn response(offset: u64, extra: &Value) -> AssistantMessage {
    serde_json::from_value(assistant("", offset, extra)).unwrap()
}

/// A response recorded now, so it is newer than every stored compaction.
fn current_response(stop: &StopReason, error: &str) -> AssistantMessage {
    let mut record = assistant("", 0, &answer_extra(stop, Some(error), 0));
    record["timestamp"] = json!(maestro_models::timestamp_now());
    serde_json::from_value(record).unwrap()
}

/// A response recorded at `timestamp`, in milliseconds since the epoch.
fn response_at(timestamp: f64, stop: &StopReason, error: &str) -> AssistantMessage {
    let mut record = assistant("", 0, &answer_extra(stop, Some(error), 0));
    record["timestamp"] = json!(timestamp);
    serde_json::from_value(record).unwrap()
}

/// The creation instant of the newest stored compaction, in milliseconds since the epoch.
fn boundary_millis(h: &Harness) -> f64 {
    let manager = h.session.session_manager();
    let manager = manager.borrow();
    let entry = manager
        .get_entries()
        .into_iter()
        .rev()
        .find(|entry| entry["type"] == "compaction")
        .expect("a stored compaction");
    let timestamp = entry["timestamp"].as_str().unwrap();
    let millis = maestro_session::messages::parse_timestamp_millis(timestamp).unwrap();
    num_traits::cast::cast::<i64, f64>(millis).unwrap()
}

/// A response that reports a context overflow.
fn overflow_response() -> AssistantMessage {
    current_response(
        &StopReason::Error,
        "prompt is too long: 300000 tokens > 200000 maximum",
    )
}

/// Appends one response to the agent's live history.
fn push_agent_message(h: &Harness, message: &AssistantMessage) {
    let messages = h.session.messages();
    messages
        .write()
        .unwrap()
        .push(maestro_extensions::NativeAgentMessage::Assistant(Arc::new(
            RwLock::new(message.clone()),
        )));
}

/// Appends a stored user message.
fn append_user(h: &Harness, text: &str, offset: u64) {
    let manager = h.session.session_manager();
    drop(
        manager
            .borrow_mut()
            .append_message(user(&json!([{"type": "text", "text": text}]), offset)),
    );
}

/// Appends the recorded pre-compaction entries and the compaction over them; returns the
/// identity the stored compaction keeps.
fn append_boundary(h: &Harness, response: &AssistantMessage, tokens: f64) -> String {
    let manager = h.session.session_manager();
    let mut manager = manager.borrow_mut();
    drop(manager.append_message(user(&json!([{"type": "text", "text": "before"}]), 50)));
    drop(manager.append_message(serde_json::to_value(response).unwrap()));
    let first_kept = manager.get_entries()[0]["id"].as_str().unwrap().to_owned();
    drop(manager.append_compaction(
        "summary",
        &first_kept,
        tokens,
        CompactionOptions {
            details: Presence::Missing,
            from_hook: Presence::Present(false),
        },
    ));
    first_kept
}

/// Replaces the agent's live history with `records`.
fn set_agent_messages(h: &Harness, records: &[Value]) {
    let messages = serde_json::from_value(json!(records)).unwrap();
    h.session
        .agent()
        .state()
        .write()
        .unwrap()
        .set_messages(messages);
}

/// The recorded form of a stored entry.
fn entry_view(entry: &Value) -> Value {
    let timestamp = entry["timestamp"]
        .as_str()
        .filter(|text| maestro_session::messages::parse_timestamp_millis(text).is_some())
        .map_or_else(|| entry["timestamp"].clone(), |_| json!("$ISO"));
    match entry["type"].as_str() {
        Some("message") => json!({
            "type": "message",
            "message": message_view(&entry["message"]),
            "timestamp": timestamp,
        }),
        Some("compaction") => json!({
            "type": "compaction",
            "summary": entry["summary"],
            "firstKeptEntryId": entry["firstKeptEntryId"],
            "tokensBefore": recorded_number(entry["tokensBefore"].as_f64().unwrap_or_default()),
            "details": entry.get("details").cloned().unwrap_or(Value::Null),
            "fromHook": entry.get("fromHook").cloned().unwrap_or(Value::Null),
            "timestamp": timestamp,
        }),
        _ => json!({"type": entry["type"], "timestamp": timestamp}),
    }
}

/// The recorded form of one message: its role, its text and its terminal outcome.
fn message_view(record: &Value) -> Value {
    let mut view = json!({"role": record["role"], "text": message_text(record)});
    for key in ["stopReason", "errorMessage"] {
        if let Some(value) = record.get(key) {
            view[key] = value.clone();
        }
    }
    view
}

/// The recorded form of the agent history.
fn messages_view(h: &Harness) -> Value {
    let messages = h.session.messages();
    let messages = messages.read().unwrap();
    Value::Array(
        messages
            .iter()
            .map(|message| message_view(&serde_json::to_value(message).unwrap()))
            .collect(),
    )
}

/// The recorded form of the stored history.
fn entries_view(h: &Harness) -> Value {
    let manager = h.session.session_manager();
    let manager = manager.borrow();
    Value::Array(manager.get_entries().into_iter().map(entry_view).collect())
}

/// A recorded view without the entry identity a compaction kept: the recorded identities are
/// a per-file sequence, and the kept entry is asserted against the recorded history instead.
fn without_kept_identity(view: &Value) -> Value {
    match view {
        Value::Array(items) => Value::Array(items.iter().map(without_kept_identity).collect()),
        Value::Object(members) => Value::Object(
            members
                .iter()
                .filter(|(key, _)| key.as_str() != "firstKeptEntryId")
                .map(|(key, value)| (key.clone(), without_kept_identity(value)))
                .collect(),
        ),
        _ => view.clone(),
    }
}

/// The events the session announced, as recorded.
fn events_view(h: &Harness) -> Value {
    without_kept_identity(&Value::Array(h.events.borrow().clone()))
}

/// Asserts the announced events, the rebuilt history and the stored entries.
fn assert_observation(h: &Harness, expected: &Value) {
    assert_eq!(events_view(h), without_kept_identity(&expected["events"]));
    assert_eq!(messages_view(h), expected["messages"]);
    assert_eq!(
        without_kept_identity(&entries_view(h)),
        without_kept_identity(&expected["entries"])
    );
}

/// The identity the newest stored compaction kept.
fn kept_entry_id(h: &Harness) -> String {
    let manager = h.session.session_manager();
    let manager = manager.borrow();
    let entry = manager
        .get_entries()
        .into_iter()
        .rev()
        .find(|entry| entry["type"] == "compaction")
        .expect("a stored compaction");
    entry["firstKeptEntryId"].as_str().unwrap().to_owned()
}

/// Asserts the compaction kept the entry the recorded history still starts after its summary.
fn assert_kept_starts_the_history(h: &Harness, messages: &Value) {
    let kept = kept_entry_id(h);
    let manager = h.session.session_manager();
    let manager = manager.borrow();
    let entry = manager
        .get_entries()
        .into_iter()
        .find(|entry| entry["id"].as_str() == Some(kept.as_str()))
        .expect("the kept entry")
        .clone();
    assert_eq!(message_view(&entry["message"]), messages[1]);
}

/// Asserts a refusal, naming the recorded message.
fn assert_refused(outcome: &Result<CompactionResult, DiagnosticErrorInfo>, expected: &Value) {
    match outcome {
        Ok(_) => panic!("the compaction was expected to be refused"),
        Err(error) => assert_eq!(error.message, expected["error"].as_str().unwrap()),
    }
}

/// A before-compaction handler the recorded cases install.
type Handler = Rc<dyn Fn(&mut SessionBeforeCompactEvent) -> Option<SessionBeforeCompactResult>>;

/// A handler's checkpoint: it reports reaching the first notifier, then answers once the
/// second is notified.
type Gate = (Rc<Notify>, Rc<Notify>);

/// The extension the recorded cases install to supply or refuse a compaction.
fn handled(handler: Handler) -> ExtensionFactory<Local> {
    gated(handler, None)
}

/// The extension answering with `handler`, after passing `gate` when one is supplied.
fn gated(handler: Handler, gate: Option<Gate>) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let handler = Rc::clone(&handler);
        let gate = gate.clone();
        let registered = api.on::<SessionBeforeCompactEvent, SessionBeforeCompactResult>(
            "session_before_compact".into(),
            Rc::new(move |event, _| {
                let result = handler(event);
                let gate = gate.clone();
                Box::pin(async move {
                    pass(gate).await;
                    Ok(result)
                })
            }),
        );
        drop(registered);
        Box::pin(std::future::ready(Ok(())))
    })
}

/// Reports reaching `gate` and waits for its release; without a gate it passes at once.
async fn pass(gate: Option<Gate>) {
    if let Some((reached, release)) = gate {
        reached.notify_one();
        release.notified().await;
    }
}

/// The handler the recorded cases install to supply the preparation's summary.
fn supplied_summary() -> Handler {
    Rc::new(|event| Some(summary_result(event, AUTO_SUMMARY)))
}

/// The summary text every recorded case supplies.
const AUTO_SUMMARY: &str = "auto compacted";

/// The extension the recorded cases install to refuse a compaction.
fn veto() -> ExtensionFactory<Local> {
    handled(Rc::new(|_| {
        Some(SessionBeforeCompactResult {
            cancel: Presence::Present(true),
            compaction: Presence::Missing,
        })
    }))
}

/// The extension the recorded cases install; its failure is reported and the default
/// summarizer runs.
fn failing() -> ExtensionFactory<Local> {
    Rc::new(move |api: ExtensionAPI<Local>| {
        let registered = api.on::<SessionBeforeCompactEvent, SessionBeforeCompactResult>(
            "session_before_compact".into(),
            Rc::new(|_, _| Box::pin(std::future::ready(Err("extension blew up".into())))),
        );
        drop(registered);
        Box::pin(std::future::ready(Ok(())))
    })
}

/// The extension the branch-summary case installs: it answers once the event is cancelled.
fn waiting_tree_extension(reached: Rc<Notify>) -> ExtensionFactory<Local> {
    Rc::new(move |api: ExtensionAPI<Local>| {
        let reached = Rc::clone(&reached);
        let registered = api.on::<SessionBeforeTreeEvent, SessionBeforeTreeResult>(
            "session_before_tree".into(),
            Rc::new(move |event, _| {
                let signal = event.signal.clone();
                reached.notify_one();
                Box::pin(async move {
                    signal.cancelled().await;
                    let result = serde_json::from_value(json!({"cancel": true})).unwrap();
                    Ok(Some(result))
                })
            }),
        );
        drop(registered);
        Box::pin(std::future::ready(Ok(())))
    })
}

/// The extension whose before-tree handler passes `gate` and lets the move go on; it counts
/// the announced moves and notifies `announced` after each.
fn gated_tree_extension(
    gate: Gate,
    moves: Rc<Cell<u32>>,
    announced: Rc<Notify>,
) -> ExtensionFactory<Local> {
    Rc::new(move |api: ExtensionAPI<Local>| {
        let gate = gate.clone();
        let before = api.on::<SessionBeforeTreeEvent, SessionBeforeTreeResult>(
            "session_before_tree".into(),
            Rc::new(move |_, _| {
                let gate = Some(gate.clone());
                Box::pin(async move {
                    pass(gate).await;
                    Ok(None)
                })
            }),
        );
        let (moves, announced) = (Rc::clone(&moves), Rc::clone(&announced));
        let after = api.on::<SessionTreeEvent, ()>(
            "session_tree".into(),
            Rc::new(move |_, _| {
                moves.set(moves.get() + 1);
                announced.notify_one();
                Box::pin(std::future::ready(Ok(None)))
            }),
        );
        drop((before, after));
        Box::pin(std::future::ready(Ok(())))
    })
}

/// An extension whose handler reports whether a compaction is in progress.
fn probing_tree_extension(
    during: Rc<Cell<bool>>,
    session: Rc<RefCell<Weak<AgentSession<Local>>>>,
) -> ExtensionFactory<Local> {
    Rc::new(move |api: ExtensionAPI<Local>| {
        let during = Rc::clone(&during);
        let session = Rc::clone(&session);
        let registered = api.on::<SessionBeforeTreeEvent, SessionBeforeTreeResult>(
            "session_before_tree".into(),
            Rc::new(move |_, _| {
                let session = session.borrow().upgrade();
                if let Some(session) = session {
                    during.set(session.is_compacting());
                }
                Box::pin(std::future::ready(Ok(None)))
            }),
        );
        drop(registered);
        Box::pin(std::future::ready(Ok(())))
    })
}

/// An extension whose handler waits for the event's signal before refusing.
fn waiting_extension(reached: Rc<Notify>) -> ExtensionFactory<Local> {
    Rc::new(move |api: ExtensionAPI<Local>| {
        let reached = Rc::clone(&reached);
        let registered = api.on::<SessionBeforeCompactEvent, SessionBeforeCompactResult>(
            "session_before_compact".into(),
            Rc::new(move |event, _| {
                let signal = event.signal.clone();
                reached.notify_one();
                Box::pin(async move {
                    signal.cancelled().await;
                    Ok(Some(SessionBeforeCompactResult {
                        cancel: Presence::Present(true),
                        compaction: Presence::Missing,
                    }))
                })
            }),
        );
        drop(registered);
        Box::pin(std::future::ready(Ok(())))
    })
}

/// The identity of the first stored entry.
fn first_entry_id(h: &Harness) -> String {
    let manager = h.session.session_manager();
    manager.borrow().get_entries()[0]["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// Navigation that leaves the branch without summarizing it.
fn navigate_options() -> maestro_session::session_manager::NavigateTreeOptions {
    maestro_session::session_manager::NavigateTreeOptions {
        summarize: Some(false),
        ..maestro_session::session_manager::NavigateTreeOptions::default()
    }
}

/// The extension that records the identity of every announced compaction entry.
fn announced(ids: Rc<RefCell<Vec<String>>>) -> ExtensionFactory<Local> {
    Rc::new(move |api: ExtensionAPI<Local>| {
        let ids = Rc::clone(&ids);
        let registered = api.on::<SessionCompactEvent, ()>(
            "session_compact".into(),
            Rc::new(move |event, _| {
                ids.borrow_mut()
                    .push(event.compaction_entry.base.id.clone());
                Box::pin(std::future::ready(Ok(None)))
            }),
        );
        drop(registered);
        Box::pin(std::future::ready(Ok(())))
    })
}

/// Notified each time the session announces an event `wanted` accepts.
fn announced_event(h: &Harness, wanted: fn(&AgentSessionEvent) -> bool) -> Rc<Notify> {
    let announced = Rc::new(Notify::new());
    let notify = Rc::clone(&announced);
    drop(h.session.subscribe(Rc::new(move |event| {
        if wanted(event) {
            notify.notify_one();
        }
    })));
    announced
}

/// Notified once the session announces a retry attempt.
fn retry_started(h: &Harness) -> Rc<Notify> {
    announced_event(h, |event| {
        matches!(event, AgentSessionEvent::AutoRetryStart { .. })
    })
}

/// Notified once the session announces the end of a compaction.
fn compaction_ended(h: &Harness) -> Rc<Notify> {
    announced_event(h, |event| {
        matches!(event, AgentSessionEvent::CompactionEnd { .. })
    })
}

// -------------------------------------------------------------- classification

#[test]
fn retry_classification_follows_the_reference_table() {
    let expected = super::session_harness::corpus("retry_classification");
    on_paused_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        for row in expected.as_array().unwrap() {
            let window = row["contextWindow"].as_f64().unwrap();
            let model = h.model("plain");
            model.write().unwrap().context_window = window;
            h.session.agent().state().write().unwrap().model = Arc::clone(&model);
            let stop = serde_json::from_value(row["stopReason"].clone()).unwrap();
            let message = faux_assistant_message(
                "",
                FauxAssistantMessageOptions {
                    stop_reason: Some(stop),
                    error_message: row["errorMessage"].as_str().map(str::to_owned),
                    ..FauxAssistantMessageOptions::default()
                },
            );
            assert_eq!(
                h.session.is_retryable_error(&message),
                row["retryable"].as_bool().unwrap(),
                "{row}"
            );
        }
    });
}

// -------------------------------------------------------------- checks

/// A summary provider that counts the compactions that reached it.
fn counted_summary(calls: Rc<Cell<usize>>) -> ExtensionFactory<Local> {
    handled(Rc::new(move |event| {
        calls.set(calls.get() + 1);
        Some(summary_result(event, AUTO_SUMMARY))
    }))
}

/// Checks one finished response reporting `tokens` and `total`, and reports how many
/// compactions started over a 100-token window.
async fn started_compactions(settings: Value, tokens: [u64; 4], total: Option<u64>) -> usize {
    let calls = Rc::new(Cell::new(0));
    let h = seeded_window(100.0, settings, vec![counted_summary(Rc::clone(&calls))]).await;
    let message = response(
        400,
        &json!({
            "stopReason": StopReason::Stop,
            "usage": usage(tokens, 0.0, total),
        }),
    );
    h.session.check_compaction(&message, true).await.unwrap();
    calls.get()
}

#[test]
fn missing_total_compaction_thresholds_use_components() {
    on_paused_task_set(|_| async {
        let threshold = json!({"compaction": {"reserveTokens": 87}});
        for (sum, expected) in [(12_u64, 0), (13, 0), (14, 1)] {
            let starts = started_compactions(threshold.clone(), [sum, 0, 0, 0], None).await;
            assert_eq!(starts, expected, "categories summing to {sum}");
        }
        let supplied = started_compactions(threshold, [14, 0, 0, 0], Some(1)).await;
        assert_eq!(supplied, 0, "a supplied total replaces the categories");
        let disabled = started_compactions(
            json!({"compaction": {"enabled": false}}),
            [14, 0, 0, 0],
            None,
        )
        .await;
        assert_eq!(disabled, 0, "disabled compaction");
    });
}

#[test]
fn disabled_compaction_emits_nothing() {
    let expected = case("compaction_checks", "disabled_setting_does_nothing");
    on_paused_task_set(|_| async {
        let h = seeded(
            json!({"compaction": {"enabled": false}}),
            vec![summary_provider(AUTO_SUMMARY)],
        )
        .await;
        let message = response(400, &answer_extra(&StopReason::Stop, None, 199_999));
        h.session.check_compaction(&message, true).await.unwrap();
        assert_observation(&h, &expected);
    });
}

#[test]
fn aborted_messages_are_skipped_unless_included() {
    let expected = case(
        "compaction_checks",
        "aborted_message_skipped_unless_included",
    );
    on_paused_task_set(|_| async {
        let h = seeded(json!({}), vec![summary_provider(AUTO_SUMMARY)]).await;
        let aborted = response(
            400,
            &answer_extra(&StopReason::Aborted, Some("aborted"), 199_999),
        );
        h.session.check_compaction(&aborted, true).await.unwrap();
        assert_observation(&h, &expected["skipped"]);
        let included = response(
            2_000,
            &answer_extra(&StopReason::Aborted, Some("aborted"), 199_999),
        );
        h.session.check_compaction(&included, false).await.unwrap();
        assert_observation(&h, &expected["included"]);
        assert_kept_starts_the_history(&h, &expected["included"]["messages"]);
    });
}

#[test]
fn overflow_recovery_is_attempted_once_then_reported() {
    let expected = case("compaction_checks", "overflow_recovery_is_attempted_once");
    on_paused_task_set(|_| async {
        let h = seeded(json!({}), vec![summary_provider(AUTO_SUMMARY)]).await;
        let overflow = overflow_response();
        set_agent_messages(
            &h,
            &[
                user(&json!([{"type": "text", "text": "one"}]), 100),
                assistant("", 200, &answer_extra(&StopReason::Stop, None, 10)),
                serde_json::to_value(&overflow).unwrap(),
            ],
        );
        h.session.check_compaction(&overflow, true).await.unwrap();
        assert_observation(&h, &expected["first"]);
        assert_kept_starts_the_history(&h, &expected["first"]["messages"]);
        let second = response_at(
            boundary_millis(&h) + 1.0,
            &StopReason::Error,
            "prompt is too long",
        );
        h.session.check_compaction(&second, true).await.unwrap();
        assert_observation(&h, &expected["second"]);
    });
}

#[test]
fn together_and_litellm_overflow_errors_start_compaction_recovery() {
    on_paused_task_set(|_| async {
        for text in [
            "The input (140000 tokens) is longer than the model's context length (131072 tokens).",
            "Requested token count exceeds the model's maximum context length of 131072 tokens",
        ] {
            let calls = Rc::new(Cell::new(0));
            let h = seeded(json!({}), vec![counted_summary(Rc::clone(&calls))]).await;
            let overflow = current_response(&StopReason::Error, text);
            set_agent_messages(
                &h,
                &[
                    user(&json!([{"type": "text", "text": "one"}]), 100),
                    assistant("", 200, &answer_extra(&StopReason::Stop, None, 10)),
                    serde_json::to_value(&overflow).unwrap(),
                ],
            );
            h.session.check_compaction(&overflow, true).await.unwrap();
            assert_eq!(calls.get(), 1, "{text}");
        }
    });
}

#[test]
fn overflow_recovery_asks_the_run_to_continue() {
    let expected = case(
        "compaction_checks",
        "overflow_recovery_continues_after_100ms",
    );
    on_paused_task_set(|_| async {
        let h = seeded(json!({}), vec![summary_provider(AUTO_SUMMARY)]).await;
        let overflow = overflow_response();
        set_agent_messages(
            &h,
            &[
                user(&json!([{"type": "text", "text": "one"}]), 100),
                assistant("", 200, &answer_extra(&StopReason::Stop, None, 10)),
                serde_json::to_value(&overflow).unwrap(),
            ],
        );
        let continues = h.session.check_compaction(&overflow, true).await.unwrap();
        assert!(continues, "the run retries the turn after the recovery");
        assert_observation(&h, &expected["before"]);
    });
}

#[test]
fn usage_before_the_compaction_boundary_is_ignored() {
    let expected = case("compaction_checks", "stale_usage_is_ignored");
    on_paused_task_set(|_| async {
        let h = seeded(json!({}), vec![summary_provider(AUTO_SUMMARY)]).await;
        let stale = response(250, &answer_extra(&StopReason::Stop, None, 199_999));
        let first_kept = append_boundary(&h, &stale, 199_999.0);
        append_user(&h, "after", 900);
        h.session.check_compaction(&stale, false).await.unwrap();
        assert_observation(&h, &expected);
        assert_eq!(kept_entry_id(&h), first_kept);
    });
}

#[test]
fn threshold_compaction_ends_without_retry() {
    let expected = case("compaction_checks", "threshold_compacts_without_retry");
    on_paused_task_set(|_| async {
        let h = seeded(json!({}), vec![summary_provider(AUTO_SUMMARY)]).await;
        let message = response(400, &answer_extra(&StopReason::Stop, None, 199_999));
        h.session.check_compaction(&message, true).await.unwrap();
        assert_observation(&h, &expected);
        assert_kept_starts_the_history(&h, &expected["messages"]);
    });
}

#[test]
fn below_threshold_usage_emits_nothing() {
    let expected = case("compaction_checks", "below_threshold_does_nothing");
    on_paused_task_set(|_| async {
        let h = seeded(
            json!({"compaction": {"reserveTokens": 1000}}),
            vec![summary_provider(AUTO_SUMMARY)],
        )
        .await;
        let message = response(400, &answer_extra(&StopReason::Stop, None, 198_000));
        h.session.check_compaction(&message, true).await.unwrap();
        assert_observation(&h, &expected);
    });
}

#[test]
fn error_messages_use_the_last_successful_usage() {
    let expected = case(
        "compaction_checks",
        "error_message_uses_last_successful_usage",
    );
    on_paused_task_set(|_| async {
        let h = seeded(json!({}), vec![summary_provider(AUTO_SUMMARY)]).await;
        let error = response(
            400,
            &answer_extra(&StopReason::Error, Some("529 overloaded"), 0),
        );
        set_agent_messages(
            &h,
            &[
                user(&json!([{"type": "text", "text": "hello"}]), 100),
                assistant("", 200, &answer_extra(&StopReason::Stop, None, 190_000)),
                user(&json!([{"type": "text", "text": "retry"}]), 300),
                serde_json::to_value(&error).unwrap(),
            ],
        );
        h.session.check_compaction(&error, true).await.unwrap();
        assert_observation(&h, &expected);
        assert_kept_starts_the_history(&h, &expected["messages"]);
    });
}

#[test]
fn zero_usage_responses_use_the_last_real_usage() {
    on_paused_task_set(|_| async {
        for (prior, expected) in [(Some(14), 1), (None, 0)] {
            let calls = Rc::new(Cell::new(0));
            let h = seeded_window(
                100.0,
                json!({"compaction": {"reserveTokens": 87}}),
                vec![counted_summary(Rc::clone(&calls))],
            )
            .await;
            let zero = response(400, &answer_extra(&StopReason::Stop, None, 0));
            let mut records = vec![user(&json!([{"type": "text", "text": "hello"}]), 100)];
            if let Some(total) = prior {
                records.push(assistant(
                    "",
                    200,
                    &answer_extra(&StopReason::Stop, None, total),
                ));
            }
            records.push(serde_json::to_value(&zero).unwrap());
            set_agent_messages(&h, &records);
            h.session.check_compaction(&zero, true).await.unwrap();
            assert_eq!(calls.get(), expected, "prior usage {prior:?}");
        }
    });
}

#[test]
fn errors_without_usage_emit_nothing() {
    let expected = case("compaction_checks", "error_without_usage_does_nothing");
    on_paused_task_set(|_| async {
        let h = seeded(json!({}), vec![summary_provider(AUTO_SUMMARY)]).await;
        let error = response(
            400,
            &answer_extra(&StopReason::Error, Some("529 overloaded"), 0),
        );
        set_agent_messages(
            &h,
            &[
                user(&json!([{"type": "text", "text": "hello"}]), 100),
                serde_json::to_value(&error).unwrap(),
            ],
        );
        h.session.check_compaction(&error, true).await.unwrap();
        assert_observation(&h, &expected);
    });
}

#[test]
fn kept_pre_compaction_usage_does_not_trigger_compaction() {
    let expected = case(
        "compaction_checks",
        "post_compaction_error_ignores_kept_usage",
    );
    on_paused_task_set(|_| async {
        let h = seeded(json!({}), vec![summary_provider(AUTO_SUMMARY)]).await;
        let kept = response(250, &answer_extra(&StopReason::Stop, None, 190_000));
        let first_kept = append_boundary(&h, &kept, 190_000.0);
        let error = response(
            1_000,
            &answer_extra(&StopReason::Error, Some("529 overloaded"), 0),
        );
        set_agent_messages(
            &h,
            &[
                user(&json!([{"type": "text", "text": "kept user"}]), 50),
                serde_json::to_value(&kept).unwrap(),
                user(&json!([{"type": "text", "text": "new prompt"}]), 900),
                serde_json::to_value(&error).unwrap(),
            ],
        );
        h.session.check_compaction(&error, true).await.unwrap();
        assert_observation(&h, &expected);
        assert_eq!(kept_entry_id(&h), first_kept);
    });
}

#[test]
fn another_models_overflow_is_not_recovered() {
    let expected = case(
        "compaction_checks",
        "foreign_model_overflow_is_not_overflow",
    );
    on_paused_task_set(|_| async {
        let h = seeded(
            json!({"compaction": {"reserveTokens": 1000}}),
            vec![summary_provider(AUTO_SUMMARY)],
        )
        .await;
        let mut record = assistant(
            "",
            400,
            &answer_extra(&StopReason::Error, Some("prompt is too long"), 199_999),
        );
        record["model"] = json!("other");
        let foreign: AssistantMessage = serde_json::from_value(record).unwrap();
        h.session.check_compaction(&foreign, true).await.unwrap();
        assert_observation(&h, &expected);
    });
}

#[test]
fn queued_messages_ask_the_run_to_continue_after_the_compaction() {
    let expected = case(
        "compaction_checks",
        "queue_only_compaction_resumes_at_100ms",
    );
    on_paused_task_set(|_| async {
        let h = seeded(json!({}), vec![summary_provider(AUTO_SUMMARY)]).await;
        let queued: CustomAgentMessages = serde_json::from_value(json!({
            "role": "custom",
            "customType": "test",
            "content": [{"type": "text", "text": "queued custom"}],
            "display": false,
            "timestamp": 500,
        }))
        .unwrap();
        h.session
            .agent()
            .follow_up(AgentMessage::Custom(Arc::new(RwLock::new(queued))));
        let message = response(400, &answer_extra(&StopReason::Stop, None, 199_999));
        let continues = h.session.check_compaction(&message, true).await.unwrap();
        assert!(continues, "the run delivers the queued message");
        assert_observation(&h, &expected["before"]);
    });
}

#[test]
fn automatic_compaction_without_credentials_ends_silently() {
    let expected = case(
        "compaction_checks",
        "auto_compaction_without_auth_ends_silently",
    );
    on_paused_task_set(|_| async {
        let h = unauthenticated(vec![summary_provider(AUTO_SUMMARY)]).await;
        seed_history(&h);
        let message = response(400, &answer_extra(&StopReason::Stop, None, 199_999));
        h.session.check_compaction(&message, true).await.unwrap();
        assert_observation(&h, &expected);
    });
}

#[test]
fn extension_cancellation_ends_the_automatic_compaction() {
    let expected = case(
        "compaction_checks",
        "auto_compaction_cancelled_by_extension",
    );
    on_paused_task_set(|_| async {
        let h = seeded(json!({}), vec![veto()]).await;
        let overflow = overflow_response();
        push_agent_message(&h, &overflow);
        h.session.check_compaction(&overflow, true).await.unwrap();
        assert_observation(&h, &expected);
    });
}

#[test]
fn automatic_compaction_over_empty_history_ends_silently() {
    let expected = case(
        "compaction_checks",
        "auto_compaction_over_empty_history_ends_silently",
    );
    on_paused_task_set(|_| async {
        let h = harness(HarnessOptions {
            settings: keep_recent(json!({})),
            factories: vec![summary_provider(AUTO_SUMMARY)],
            context_window: Some(WINDOW),
            ..HarnessOptions::default()
        })
        .await;
        let message = response(400, &answer_extra(&StopReason::Stop, None, 199_999));
        h.session.check_compaction(&message, true).await.unwrap();
        assert_observation(&h, &expected);
    });
}

#[test]
fn automatic_failures_report_their_reason() {
    let expected = case(
        "compaction_checks",
        "auto_compaction_failures_report_reason",
    );
    on_paused_task_set(|_| async {
        let h = seeded(json!({}), vec![failing()]).await;
        let overflow = overflow_response();
        push_agent_message(&h, &overflow);
        h.session.check_compaction(&overflow, true).await.unwrap();
        let message = response(400, &answer_extra(&StopReason::Stop, None, 199_999));
        h.session.check_compaction(&message, true).await.unwrap();
        assert_observation(&h, &expected);
    });
}

#[test]
fn automatic_start_event_precedes_the_abort_controller() {
    let expected = case(
        "compaction_checks",
        "auto_start_event_precedes_the_abort_controller",
    );
    on_paused_task_set(|_| async {
        let h = seeded(json!({}), vec![summary_provider(AUTO_SUMMARY)]).await;
        let session = Rc::downgrade(&h.session);
        let stopped = Rc::new(Cell::new(false));
        let flag = Rc::clone(&stopped);
        drop(h.session.subscribe(Rc::new(move |event| {
            if matches!(
                event,
                AgentSessionEvent::CompactionStart {
                    reason: CompactionReason::Threshold
                }
            ) && let Some(session) = session.upgrade()
            {
                flag.set(true);
                session.abort_compaction();
            }
        })));
        let message = response(400, &answer_extra(&StopReason::Stop, None, 199_999));
        h.session.check_compaction(&message, true).await.unwrap();
        assert!(stopped.get(), "the listener saw the start event");
        assert_observation(&h, &expected);
        assert_kept_starts_the_history(&h, &expected["messages"]);
    });
}

#[test]
fn branch_summary_reports_a_running_compaction() {
    let expected = case(
        "compaction_checks",
        "abort_branch_summary_reaches_the_navigation_handler",
    );
    on_paused_task_set(|_| async {
        let during = Rc::new(Cell::new(false));
        let probe = Rc::new(RefCell::new(Weak::new()));
        let h = seeded(
            json!({}),
            vec![probing_tree_extension(
                Rc::clone(&during),
                Rc::clone(&probe),
            )],
        )
        .await;
        *probe.borrow_mut() = Rc::downgrade(&h.session);
        drop(
            h.session
                .session_manager()
                .borrow_mut()
                .append_message(user(&json!([{"type": "text", "text": "three"}]), 400)),
        );
        let target = first_entry_id(&h);
        let outcome = h
            .session
            .navigate_tree(&target, navigate_options())
            .await
            .unwrap();
        assert_eq!(during.get(), expected["compactingDuring"] == true);
        assert!(!outcome.cancelled);
        assert_eq!(
            h.session.is_compacting(),
            expected["isCompactingAfter"] == true
        );
        assert_eq!(
            h.session.session_manager().borrow().get_entries().len() as u64,
            expected["entryCount"].as_u64().unwrap()
        );
        assert!(!h.session.is_compacting());
    });
}

#[test]
fn abort_branch_summary_reaches_the_navigation_handler() {
    let expected = case(
        "compaction_checks",
        "abort_branch_summary_reaches_the_navigation_handler",
    );
    on_paused_task_set(|_| async {
        let reached = Rc::new(Notify::new());
        let h = seeded(json!({}), vec![waiting_tree_extension(Rc::clone(&reached))]).await;
        let manager = h.session.session_manager();
        drop(
            manager
                .borrow_mut()
                .append_message(user(&json!([{"type": "text", "text": "three"}]), 400)),
        );
        let target = manager.borrow().get_entries()[0]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let session = Rc::clone(&h.session);
        let task = tokio::task::spawn_local(async move {
            session.navigate_tree(&target, navigate_options()).await
        });
        reached.notified().await;
        assert_eq!(
            h.session.is_compacting(),
            expected["compactingDuring"] == true
        );
        h.session.abort_branch_summary();
        let outcome = task.await.unwrap().unwrap();
        assert_eq!(
            outcome.editor_text.as_deref(),
            expected["outcome"]["editorText"].as_str()
        );
        assert_eq!(
            outcome.cancelled,
            expected["outcome"]["cancelled"].as_bool().unwrap()
        );
        assert_eq!(outcome.aborted, expected["outcome"]["aborted"] == true);
        assert_eq!(
            h.session.is_compacting(),
            expected["isCompactingAfter"] == true
        );
    });
}

// -------------------------------------------------------------- manual

#[test]
fn manual_compaction_uses_the_extension_summary() {
    let expected = case(
        "manual_compaction",
        "manual_compaction_uses_extension_summary",
    );
    on_paused_task_set(|_| async {
        let h = seeded(json!({}), vec![summary_provider(AUTO_SUMMARY)]).await;
        let result = h.session.compact(None).await.unwrap();
        assert_eq!(
            result.summary,
            expected["result"]["summary"].as_str().unwrap()
        );
        assert_eq!(
            recorded_number(result.tokens_before),
            expected["result"]["tokensBefore"]
        );
        assert_eq!(
            result.details.present().unwrap(),
            &expected["result"]["details"]
        );
        assert_observation(&h, &expected);
        assert_kept_starts_the_history(&h, &expected["messages"]);
    });
}

#[test]
fn manual_compaction_generates_and_stores_a_summary() {
    let expected = case("manual_compaction", "manual_compaction_generates_summary");
    on_paused_task_set(|_| async {
        let h = seeded(json!({}), Vec::new()).await;
        let prompts = script_responses(&h, vec![answer("generated summary", None, None)]);
        let result = h.session.compact(Some("focus on files")).await.unwrap();
        assert_eq!(
            result.summary,
            expected["result"]["summary"].as_str().unwrap()
        );
        assert_eq!(
            recorded_number(result.tokens_before),
            expected["result"]["tokensBefore"]
        );
        assert_eq!(
            result.details.present().cloned().unwrap(),
            expected["result"]["details"]
        );
        assert_eq!(prompts.lock().unwrap().len(), 1);
        assert!(
            prompts.lock().unwrap()[0].contains("focus on files"),
            "the caller's instructions reach the summarizer"
        );
        assert_observation(&h, &expected);
        assert_kept_starts_the_history(&h, &expected["messages"]);
    });
}

#[test]
fn compacting_twice_reports_already_compacted() {
    let expected = case(
        "manual_compaction",
        "manual_compaction_twice_reports_already_compacted",
    );
    on_paused_task_set(|_| async {
        let h = seeded(json!({}), vec![summary_provider(AUTO_SUMMARY)]).await;
        drop(h.session.compact(None).await.unwrap());
        h.events.borrow_mut().clear();
        let refused = h.session.compact(None).await;
        assert_refused(&refused, &expected["second"]);
        assert_observation(&h, &expected);
    });
}

#[test]
fn compacting_empty_history_reports_nothing_to_compact() {
    let expected = case(
        "manual_compaction",
        "manual_compaction_over_empty_history_reports_nothing",
    );
    on_paused_task_set(|_| async {
        let h = harness(HarnessOptions {
            settings: keep_recent(json!({})),
            factories: vec![summary_provider(AUTO_SUMMARY)],
            context_window: Some(WINDOW),
            ..HarnessOptions::default()
        })
        .await;
        let refused = h.session.compact(None).await;
        assert_refused(&refused, &expected["refused"]);
        assert_observation(&h, &expected);
    });
}

#[test]
fn manual_compaction_without_credentials_reports_the_key_message() {
    let expected = case(
        "manual_compaction",
        "manual_compaction_without_auth_reports_key_message",
    );
    on_paused_task_set(|_| async {
        let h = unauthenticated(Vec::new()).await;
        seed_history(&h);
        let refused = h.session.compact(None).await;
        let docs = maestro_app::configuration::config::get_docs_path().unwrap();
        match refused {
            Ok(_) => panic!("the compaction was expected to be refused"),
            Err(error) => assert_eq!(
                as_recorded(json!(error.message), &[(&docs, "$DOCS")]),
                expected["refused"]["error"]
            ),
        }
        assert_eq!(
            as_recorded(events_view(&h), &[(&docs, "$DOCS")]),
            without_kept_identity(&expected["events"])
        );
        assert_eq!(messages_view(&h), expected["messages"]);
        assert_eq!(entries_view(&h), expected["entries"]);
    });
}

#[test]
fn extension_veto_ends_the_manual_compaction() {
    let expected = case("manual_compaction", "manual_compaction_vetoed_by_extension");
    on_paused_task_set(|_| async {
        let h = seeded(json!({}), vec![veto()]).await;
        let refused = h.session.compact(None).await;
        assert_refused(&refused, &expected["refused"]);
        assert_observation(&h, &expected);
    });
}

#[test]
fn aborting_during_an_extension_handler_cancels_the_compaction() {
    let expected = case(
        "manual_compaction",
        "abort_compaction_cancels_manual_compaction",
    );
    on_paused_task_set(|_| async {
        let reached = Rc::new(Notify::new());
        let h = seeded(json!({}), vec![waiting_extension(Rc::clone(&reached))]).await;
        let session = Rc::clone(&h.session);
        let task = tokio::task::spawn_local(async move { session.compact(None).await });
        reached.notified().await;
        h.session.abort_compaction();
        let outcome = task.await.unwrap();
        assert_refused(&outcome, &expected["outcome"]);
        assert_observation(&h, &expected);
    });
}

#[test]
fn summarizer_failures_report_the_prefixed_error() {
    let expected = case(
        "manual_compaction",
        "manual_compaction_failure_reports_prefixed_error",
    );
    on_paused_task_set(|_| async {
        let h = seeded(json!({}), Vec::new()).await;
        drop(script_responses(
            &h,
            vec![answer("", Some(StopReason::Error), Some("summarizer down"))],
        ));
        let refused = h.session.compact(None).await;
        assert_refused(&refused, &expected["refused"]);
        assert_observation(&h, &expected);
    });
}

#[test]
fn manual_start_event_precedes_the_abort_controller() {
    let expected = case(
        "manual_compaction",
        "manual_start_event_precedes_the_abort_controller",
    );
    on_paused_task_set(|_| async {
        let h = seeded(json!({}), vec![summary_provider(AUTO_SUMMARY)]).await;
        let session = Rc::downgrade(&h.session);
        let stopped = Rc::new(Cell::new(false));
        let flag = Rc::clone(&stopped);
        drop(h.session.subscribe(Rc::new(move |event| {
            if matches!(
                event,
                AgentSessionEvent::CompactionStart {
                    reason: CompactionReason::Manual
                }
            ) && let Some(session) = session.upgrade()
            {
                flag.set(true);
                session.abort_compaction();
            }
        })));
        let refused = h.session.compact(None).await;
        assert!(stopped.get(), "the listener saw the start event");
        assert_refused(&refused, &expected["refused"]);
        assert_observation(&h, &expected);
    });
}

// -------------------------------------------------------------- retry

#[test]
fn retries_exhaust_their_attempts_with_doubling_delays() {
    let expected = case("retry_behaviour", "retry_exhausts_attempts");
    let settings = json!({"retry": {"enabled": true, "maxRetries": 2, "baseDelayMs": 2000}});
    on_paused_task_set(|_| async {
        let h = harness(HarnessOptions {
            settings,
            ..HarnessOptions::default()
        })
        .await;
        let failing = response(
            400,
            &answer_extra(&StopReason::Error, Some("overloaded_error"), 0),
        );
        assert_eq!(
            h.session.handle_retryable_error(&failing).await,
            expected["afterFirst"]["returned"].as_bool().unwrap()
        );
        assert_observation(&h, &expected["afterFirst"]);
        assert_eq!(
            h.session.handle_retryable_error(&failing).await,
            expected["second"].as_bool().unwrap()
        );
        assert_eq!(
            h.session.handle_retryable_error(&failing).await,
            expected["third"].as_bool().unwrap()
        );
        assert_observation(&h, &expected);
        assert_eq!(
            h.session.retry_attempt(),
            u32::try_from(expected["retryAttemptAfter"].as_u64().unwrap()).unwrap()
        );
        assert_eq!(h.session.is_retrying(), expected["isRetryingAfter"] == true);
    });
}

#[test]
fn disabled_retries_do_nothing() {
    let expected = case("retry_behaviour", "retry_disabled_returns_false");
    on_paused_task_set(|_| async {
        let h = harness(HarnessOptions {
            settings: json!({"retry": {"enabled": false, "maxRetries": 2, "baseDelayMs": 2000}}),
            ..HarnessOptions::default()
        })
        .await;
        let failing = response(
            400,
            &answer_extra(&StopReason::Error, Some("overloaded_error"), 0),
        );
        assert_eq!(
            h.session.handle_retryable_error(&failing).await,
            expected["returned"].as_bool().unwrap()
        );
        assert_observation(&h, &expected);
    });
}

#[test]
fn aborting_during_backoff_ends_the_retry() {
    let expected = case(
        "retry_behaviour",
        "abort_during_backoff_cancels_the_attempt",
    );
    on_paused_task_set(|_| async {
        let h = harness(HarnessOptions {
            settings: json!({"retry": {"enabled": true, "maxRetries": 3, "baseDelayMs": 2000}}),
            ..HarnessOptions::default()
        })
        .await;
        let failing = response(
            400,
            &answer_extra(&StopReason::Error, Some("overloaded_error"), 0),
        );
        let started = retry_started(&h);
        let session = Rc::clone(&h.session);
        let task =
            tokio::task::spawn_local(async move { session.handle_retryable_error(&failing).await });
        started.notified().await;
        h.session.abort_retry();
        assert_eq!(task.await.unwrap(), expected["returned"].as_bool().unwrap());
        assert_observation(&h, &expected);
        assert_eq!(
            h.session.retry_attempt(),
            u32::try_from(expected["retryAttemptAfter"].as_u64().unwrap()).unwrap()
        );
        assert_eq!(h.session.is_retrying(), expected["isRetryingAfter"] == true);
    });
}

#[test]
fn a_retry_returns_once_the_backoff_ends() {
    let expected = case("retry_behaviour", "retry_schedules_continuation");
    on_paused_task_set(|_| async {
        let h = harness(HarnessOptions {
            settings: json!({"retry": {"enabled": true, "maxRetries": 3, "baseDelayMs": 2000}}),
            ..HarnessOptions::default()
        })
        .await;
        let failing = response(
            400,
            &answer_extra(&StopReason::Error, Some("connection lost"), 0),
        );
        let mut work = Box::pin(h.session.handle_retryable_error(&failing));
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        assert!(std::future::Future::poll(work.as_mut(), &mut context).is_pending());
        tokio::time::advance(std::time::Duration::from_millis(1_999)).await;
        assert!(std::future::Future::poll(work.as_mut(), &mut context).is_pending());
        tokio::time::advance(std::time::Duration::from_millis(1)).await;
        assert_eq!(work.await, expected["returned"].as_bool().unwrap());
        assert_eq!(events_view(&h), expected["events"]);
    });
}

#[test]
fn fractional_attempt_limits_compare_as_numbers() {
    let expected = case("retry_behaviour", "fractional_limit_compares_as_a_number");
    on_paused_task_set(|_| async {
        let h = harness(HarnessOptions {
            settings: json!({"retry": {"enabled": true, "maxRetries": 2.5, "baseDelayMs": 1}}),
            ..HarnessOptions::default()
        })
        .await;
        let failing = response(400, &answer_extra(&StopReason::Error, Some("503"), 0));
        let mut outcomes = Vec::new();
        for _ in 0..5 {
            outcomes.push(h.session.handle_retryable_error(&failing).await);
        }
        assert_eq!(json!(outcomes), expected["outcomes"]);
        assert_observation(&h, &expected);
    });
}

#[test]
fn zero_base_delay_retries_without_waiting() {
    let expected = case("retry_behaviour", "zero_base_delay_retries_immediately");
    on_paused_task_set(|_| async {
        let h = harness(HarnessOptions {
            settings: json!({"retry": {"enabled": true, "maxRetries": 1, "baseDelayMs": 0}}),
            ..HarnessOptions::default()
        })
        .await;
        let failing = response(400, &answer_extra(&StopReason::Error, Some("502"), 0));
        assert_eq!(
            h.session.handle_retryable_error(&failing).await,
            expected["returned"].as_bool().unwrap()
        );
        assert_observation(&h, &expected);
    });
}

#[test]
fn negative_base_delays_fire_immediately() {
    let expected = case("retry_behaviour", "negative_base_delay_fires_immediately");
    on_paused_task_set(|_| async {
        let h = harness(HarnessOptions {
            settings: json!({"retry": {"enabled": true, "maxRetries": 2, "baseDelayMs": -5}}),
            ..HarnessOptions::default()
        })
        .await;
        let failing = response(400, &answer_extra(&StopReason::Error, Some("503"), 0));
        assert_eq!(
            h.session.handle_retryable_error(&failing).await,
            expected["returned"].as_bool().unwrap()
        );
        assert_observation(&h, &expected);
    });
}

#[test]
fn toggles_persist_to_the_settings() {
    let expected = case("retry_behaviour", "toggles_write_settings");
    on_paused_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        h.session.set_auto_retry_enabled(false).unwrap();
        h.session.set_auto_compaction_enabled(false).unwrap();
        let settings = h.session.settings_manager();
        let settings = settings.borrow();
        assert_eq!(
            h.session.auto_retry_enabled(),
            expected["autoRetryEnabled"] == true
        );
        assert_eq!(
            h.session.auto_compaction_enabled(),
            expected["autoCompactionEnabled"] == true
        );
        assert_eq!(
            settings.get_retry_enabled(),
            expected["retryEnabled"] == true
        );
        assert_eq!(
            settings.get_compaction_enabled(),
            expected["compactionEnabled"] == true
        );
        assert_eq!(h.session.is_compacting(), expected["isCompacting"] == true);
    });
}

#[test]
fn aborting_cancels_a_retry_in_flight() {
    let expected = case(
        "retry_behaviour",
        "abort_during_backoff_cancels_the_attempt",
    );
    on_paused_task_set(|_| async {
        let h = harness(HarnessOptions {
            settings: json!({"retry": {"enabled": true, "maxRetries": 3, "baseDelayMs": 2000}}),
            ..HarnessOptions::default()
        })
        .await;
        let failing = response(
            400,
            &answer_extra(&StopReason::Error, Some("overloaded_error"), 0),
        );
        let started = retry_started(&h);
        let session = Rc::clone(&h.session);
        let task =
            tokio::task::spawn_local(async move { session.handle_retryable_error(&failing).await });
        started.notified().await;
        h.session.abort().await;
        assert_eq!(task.await.unwrap(), expected["returned"].as_bool().unwrap());
        assert_observation(&h, &expected);
        assert_eq!(h.session.is_retrying(), expected["isRetryingAfter"] == true);
    });
}

// -------------------------------------------------------------- settlement

/// The before-compaction extension that edits the preparation and lets generation continue.
fn editing_preparation(tokens_before: f64) -> ExtensionFactory<Local> {
    handled(Rc::new(move |event| {
        event.data.preparation.tokens_before = tokens_before;
        None
    }))
}

#[test]
fn automatic_storage_failures_end_the_compaction() {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path().display().to_string();
    let sessions = format!("{root}/sessions");
    on_paused_task_set(|_| async {
        let manager =
            SessionManager::create(&format!("{root}/project"), Some(&sessions), &root).unwrap();
        let h = harness(HarnessOptions {
            settings: keep_recent(json!({})),
            factories: vec![summary_provider(AUTO_SUMMARY)],
            context_window: Some(WINDOW),
            session_manager: Some(manager),
            ..HarnessOptions::default()
        })
        .await;
        seed_history(&h);
        std::fs::remove_dir_all(&sessions).unwrap();
        let message = response(400, &answer_extra(&StopReason::Stop, None, 199_999));
        h.session.check_compaction(&message, true).await.unwrap();
        let events = events_view(&h);
        let [start, end] = events.as_array().unwrap().as_slice() else {
            panic!("one start and one end event: {events}");
        };
        assert_eq!(
            start,
            &json!({"type": "compaction_start", "reason": "threshold"})
        );
        let message = end["errorMessage"].as_str().unwrap();
        assert!(message.starts_with("Auto-compaction failed: "), "{message}");
        assert_eq!(end["aborted"], false);
        assert_eq!(end["willRetry"], false);
        assert_eq!(end["result"], Value::Null);
        assert!(!h.session.is_compacting());
    });
}

#[test]
fn end_listeners_see_the_compaction_running() {
    on_paused_task_set(|_| async {
        let h = seeded(json!({}), vec![summary_provider(AUTO_SUMMARY)]).await;
        let session = Rc::downgrade(&h.session);
        let running = Rc::new(RefCell::new(Vec::new()));
        let seen = Rc::clone(&running);
        drop(h.session.subscribe(Rc::new(move |event| {
            if let AgentSessionEvent::CompactionEnd { reason, .. } = event
                && let Some(session) = session.upgrade()
            {
                seen.borrow_mut().push((*reason, session.is_compacting()));
            }
        })));
        let message = response(400, &answer_extra(&StopReason::Stop, None, 199_999));
        h.session.check_compaction(&message, true).await.unwrap();
        drop(h.session.compact(None).await);
        assert_eq!(
            *running.borrow(),
            [
                (CompactionReason::Threshold, true),
                (CompactionReason::Manual, true)
            ]
        );
        assert!(!h.session.is_compacting());
    });
}

#[test]
fn overflow_recovery_keeps_a_successful_stored_tail() {
    on_paused_task_set(|_| async {
        let h = seeded(json!({}), vec![summary_provider(AUTO_SUMMARY)]).await;
        let kept = assistant(
            "kept answer",
            350,
            &answer_extra(&StopReason::Stop, None, 10),
        );
        drop(
            h.session
                .session_manager()
                .borrow_mut()
                .append_message(kept.clone()),
        );
        let overflow = overflow_response();
        set_agent_messages(
            &h,
            &[
                user(&json!([{"type": "text", "text": "two"}]), 300),
                kept,
                serde_json::to_value(&overflow).unwrap(),
            ],
        );
        h.session.check_compaction(&overflow, true).await.unwrap();
        let messages = messages_view(&h);
        assert_eq!(
            messages.as_array().unwrap().last(),
            Some(&json!({"role": "assistant", "text": "kept answer", "stopReason": "stop"}))
        );
    });
}

#[test]
fn handler_edits_to_the_preparation_reach_the_summarizer() {
    on_paused_task_set(|_| async {
        let manual = seeded(json!({}), vec![editing_preparation(4242.0)]).await;
        drop(script_responses(
            &manual,
            vec![answer("generated summary", None, None)],
        ));
        let result = manual.session.compact(None).await.unwrap();
        assert_eq!(recorded_number(result.tokens_before), json!(4242));
        let automatic = seeded(json!({}), vec![editing_preparation(4242.0)]).await;
        drop(script_responses(
            &automatic,
            vec![answer("generated summary", None, None)],
        ));
        let message = response(400, &answer_extra(&StopReason::Stop, None, 199_999));
        automatic
            .session
            .check_compaction(&message, true)
            .await
            .unwrap();
        assert_eq!(events_view(&automatic)[1]["result"]["tokensBefore"], 4242);
    });
}

#[test]
fn automatic_compaction_uses_the_preferences_from_before_its_start() {
    on_paused_task_set(|_| async {
        let observed = Rc::new(Cell::new(0.0));
        let seen = Rc::clone(&observed);
        let recording = handled(Rc::new(move |event| {
            seen.set(event.data.preparation.settings.keep_recent_tokens);
            None
        }));
        let h = seeded(json!({}), vec![recording]).await;
        drop(script_responses(
            &h,
            vec![answer("generated summary", None, None)],
        ));
        let session = Rc::downgrade(&h.session);
        drop(h.session.subscribe(Rc::new(move |event| {
            if matches!(event, AgentSessionEvent::CompactionStart { .. })
                && let Some(session) = session.upgrade()
            {
                let changed = json!({"compaction": {"keepRecentTokens": 5}});
                session
                    .settings_manager()
                    .borrow_mut()
                    .apply_overrides(serde_json::from_value(changed).unwrap());
            }
        })));
        let message = response(400, &answer_extra(&StopReason::Stop, None, 199_999));
        h.session.check_compaction(&message, true).await.unwrap();
        assert_eq!(recorded_number(observed.get()), json!(1));
    });
}

#[test]
fn repeated_summaries_announce_the_newest_entry() {
    on_paused_task_set(|_| async {
        let ids = Rc::new(RefCell::new(Vec::new()));
        let h = seeded(
            json!({}),
            vec![summary_provider(AUTO_SUMMARY), announced(Rc::clone(&ids))],
        )
        .await;
        drop(h.session.compact(None).await.unwrap());
        append_user(&h, "three", 400);
        drop(
            h.session
                .session_manager()
                .borrow_mut()
                .append_message(assistant(
                    "",
                    500,
                    &answer_extra(&StopReason::Stop, None, 10),
                )),
        );
        drop(h.session.compact(None).await.unwrap());
        let stored: Vec<String> = h
            .session
            .session_manager()
            .borrow()
            .get_entries()
            .into_iter()
            .filter(|entry| entry["type"] == "compaction")
            .map(|entry| entry["id"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(stored.len(), 2);
        assert_eq!(*ids.borrow(), stored);
    });
}

#[test]
fn a_manual_compaction_left_unobserved_still_completes() {
    let expected = case(
        "manual_compaction",
        "manual_compaction_uses_extension_summary",
    );
    on_paused_task_set(|_| async {
        let gate: Gate = (Rc::new(Notify::new()), Rc::new(Notify::new()));
        let provider = gated(supplied_summary(), Some(gate.clone()));
        let h = seeded(json!({}), vec![provider]).await;
        let ended = compaction_ended(&h);
        drop(h.session.compact(None));
        assert!(h.session.is_compacting(), "the compaction still runs");
        gate.0.notified().await;
        gate.1.notify_one();
        ended.notified().await;
        assert_observation(&h, &expected);
        assert_kept_starts_the_history(&h, &expected["messages"]);
        assert!(!h.session.is_compacting());
    });
}

#[test]
fn an_automatic_compaction_left_unobserved_still_completes() {
    let expected = case("compaction_checks", "threshold_compacts_without_retry");
    on_paused_task_set(|_| async {
        let gate: Gate = (Rc::new(Notify::new()), Rc::new(Notify::new()));
        let provider = gated(supplied_summary(), Some(gate.clone()));
        let h = seeded(json!({}), vec![provider]).await;
        let ended = compaction_ended(&h);
        let message = response(400, &answer_extra(&StopReason::Stop, None, 199_999));
        start_and_abandon(h.session.check_compaction(&message, true));
        gate.0.notified().await;
        assert!(h.session.is_compacting(), "the compaction still runs");
        gate.1.notify_one();
        ended.notified().await;
        assert_observation(&h, &expected);
        assert_kept_starts_the_history(&h, &expected["messages"]);
        assert!(!h.session.is_compacting());
    });
}

#[test]
fn navigation_handler_and_abort_precede_observation() {
    on_paused_task_set(|_| async {
        let reached = Rc::new(Notify::new());
        let h = seeded(json!({}), vec![waiting_tree_extension(Rc::clone(&reached))]).await;
        let manager = h.session.session_manager();
        let old_leaf = manager.borrow().get_leaf_id().map(str::to_owned);
        let target = manager.borrow().get_entries()[1]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        let observer = h.session.navigate_tree(&target, navigate_options());
        let mut entered = std::pin::pin!(reached.notified());
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        let entered_at_call = std::future::Future::poll(entered.as_mut(), &mut context).is_ready();
        assert_eq!((entered_at_call, h.session.is_compacting()), (true, true));
        h.session.abort_branch_summary();
        let outcome = observer.await.unwrap();
        assert!(outcome.cancelled);
        assert_eq!(manager.borrow().get_leaf_id(), old_leaf.as_deref());
        assert!(!h.session.is_compacting());
    });
}

#[test]
fn a_tree_navigation_left_unobserved_still_moves() {
    on_paused_task_set(|_| async {
        let gate: Gate = (Rc::new(Notify::new()), Rc::new(Notify::new()));
        let (moves, announced) = (Rc::new(Cell::new(0)), Rc::new(Notify::new()));
        let extension =
            gated_tree_extension(gate.clone(), Rc::clone(&moves), Rc::clone(&announced));
        let h = seeded(json!({}), vec![extension]).await;
        let manager = h.session.session_manager();
        let target = manager.borrow().get_entries()[1]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        drop(h.session.navigate_tree(&target, navigate_options()));
        assert!(h.session.is_compacting(), "the navigation still runs");
        gate.0.notified().await;
        gate.1.notify_one();
        announced.notified().await;
        assert_eq!(moves.get(), 1);
        assert_eq!(manager.borrow().get_leaf_id(), Some(target.as_str()));
        assert!(!h.session.is_compacting());
    });
}

#[test]
fn aborting_a_compaction_left_unobserved_cancels_it() {
    let expected = case(
        "manual_compaction",
        "abort_compaction_cancels_manual_compaction",
    );
    on_paused_task_set(|_| async {
        let reached = Rc::new(Notify::new());
        let h = seeded(json!({}), vec![waiting_extension(Rc::clone(&reached))]).await;
        let ended = compaction_ended(&h);
        start_and_abandon(h.session.compact(None));
        reached.notified().await;
        assert!(h.session.is_compacting(), "the compaction still runs");
        h.session.abort_compaction();
        ended.notified().await;
        assert_observation(&h, &expected);
        assert!(!h.session.is_compacting());
    });
}

#[test]
fn an_automatic_compaction_whose_task_is_discarded_reports_it() {
    on_paused_task_set(|_| async {
        let tasks = Rc::new(RefCell::new(Vec::new()));
        let queue = Rc::clone(&tasks);
        let gate: Gate = (Rc::new(Notify::new()), Rc::new(Notify::new()));
        let h = harness(HarnessOptions {
            settings: keep_recent(json!({})),
            factories: vec![gated(supplied_summary(), Some(gate))],
            context_window: Some(WINDOW),
            spawn: Some(Rc::new(move |task| queue.borrow_mut().push(task))),
            ..HarnessOptions::default()
        })
        .await;
        seed_history(&h);
        let message = response(400, &answer_extra(&StopReason::Stop, None, 199_999));
        let mut check = std::pin::pin!(h.session.check_compaction(&message, true));
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        assert!(check.as_mut().poll(&mut context).is_pending());
        assert_eq!(
            tasks.borrow().len(),
            1,
            "the agent's own facility holds the work"
        );
        tasks.borrow_mut().clear();
        let std::task::Poll::Ready(outcome) = check.as_mut().poll(&mut context) else {
            panic!("the discarded compaction is reported at once");
        };
        assert_eq!(outcome.unwrap_err().message, "channel closed");
    });
}
