//! Compacting a conversation and retrying a failed response, compared with the recorded
//! recovery cases: the announced events, the rebuilt history and the stored entries.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    Auth, Harness, HarnessOptions, answer, assistant, case, harness, load_history, message_text,
    on_paused_task_set, recorded_number, script_responses, usage, user,
};
use maestro_agent::{AgentEvent, AgentMessage, PromptInput};
use maestro_app::agent_session::CompactionReason;
use maestro_app::{AgentSession, AgentSessionEvent};
use maestro_extensions::{
    ExtensionAPI, ExtensionFactory, SessionBeforeCompactEvent, SessionBeforeCompactResult,
    SessionBeforeTreeEvent, SessionBeforeTreeResult, SessionCompactEvent,
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
    let h = harness(HarnessOptions {
        settings: keep_recent(settings),
        factories,
        context_window: Some(WINDOW),
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

/// The extension the recorded cases install to supply or refuse a compaction.
fn handled(handler: Handler) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let handler = Rc::clone(&handler);
        let registered = api.on::<SessionBeforeCompactEvent, SessionBeforeCompactResult>(
            "session_before_compact".into(),
            Rc::new(move |event, _| {
                let result = handler(event);
                Box::pin(std::future::ready(Ok(result)))
            }),
        );
        drop(registered);
        Box::pin(std::future::ready(Ok(())))
    })
}

/// The extension the recorded cases install to supply the preparation's summary.
fn summary_provider() -> ExtensionFactory<Local> {
    handled(Rc::new(|event| {
        let data = &event.data;
        Some(SessionBeforeCompactResult {
            cancel: Presence::Missing,
            compaction: Presence::Present(CompactionResult {
                summary: "auto compacted".to_owned(),
                first_kept_entry_id: data.preparation.first_kept_entry_id.clone(),
                tokens_before: data.preparation.tokens_before,
                details: Presence::Present(json!({})),
            }),
        })
    }))
}

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

/// Navigation that summarizes the branch it leaves.
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

/// Notified once the session announces a retry attempt.
fn retry_started(h: &Harness) -> Rc<Notify> {
    let started = Rc::new(Notify::new());
    let notify = Rc::clone(&started);
    drop(h.session.subscribe(Rc::new(move |event| {
        if matches!(event, AgentSessionEvent::AutoRetryStart { .. }) {
            notify.notify_one();
        }
    })));
    started
}

/// Notified once an agent run ends.
fn agent_ended(h: &Harness) -> Rc<Notify> {
    let ended = Rc::new(Notify::new());
    let notify = Rc::clone(&ended);
    drop(h.session.agent().subscribe(Rc::new(move |event, _| {
        if matches!(*event.read().unwrap(), AgentEvent::AgentEnd { .. }) {
            notify.notify_one();
        }
        Box::pin(std::future::ready(Ok(())))
    })));
    ended
}

/// Drops the operation `task` runs mid-flight, returning once it is gone.
async fn drop_mid_flight<T: 'static>(task: tokio::task::JoinHandle<T>) {
    task.abort();
    assert!(task.await.err().is_some_and(|error| error.is_cancelled()));
}

// -------------------------------------------------------------- classification

#[test]
fn retry_classification_follows_the_reference_table() {
    let expected = crate::support::session_harness::corpus("retry_classification");
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

#[test]
fn disabled_compaction_emits_nothing() {
    let expected = case("compaction_checks", "disabled_setting_does_nothing");
    on_paused_task_set(|_| async {
        let h = seeded(
            json!({"compaction": {"enabled": false}}),
            vec![summary_provider()],
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
        let h = seeded(json!({}), vec![summary_provider()]).await;
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
        let h = seeded(json!({}), vec![summary_provider()]).await;
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
fn overflow_recovery_continues_after_one_hundred_milliseconds() {
    let expected = case(
        "compaction_checks",
        "overflow_recovery_continues_after_100ms",
    );
    on_paused_task_set(|_| async {
        let h = seeded(json!({}), vec![summary_provider()]).await;
        let overflow = overflow_response();
        set_agent_messages(
            &h,
            &[
                user(&json!([{"type": "text", "text": "one"}]), 100),
                assistant("", 200, &answer_extra(&StopReason::Stop, None, 10)),
                serde_json::to_value(&overflow).unwrap(),
            ],
        );
        let ended = agent_ended(&h);
        h.session.check_compaction(&overflow, true).await.unwrap();
        assert_observation(&h, &expected["before"]);
        tokio::time::advance(std::time::Duration::from_millis(99)).await;
        assert_eq!(messages_view(&h), expected["before"]["messages"]);
        drop(script_responses(
            &h,
            vec![answer("after recovery", None, None)],
        ));
        tokio::time::advance(std::time::Duration::from_millis(1)).await;
        ended.notified().await;
        assert_eq!(messages_view(&h), expected["after"]["messages"]);
    });
}

#[test]
fn usage_before_the_compaction_boundary_is_ignored() {
    let expected = case("compaction_checks", "stale_usage_is_ignored");
    on_paused_task_set(|_| async {
        let h = seeded(json!({}), vec![summary_provider()]).await;
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
        let h = seeded(json!({}), vec![summary_provider()]).await;
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
            vec![summary_provider()],
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
        let h = seeded(json!({}), vec![summary_provider()]).await;
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
fn errors_without_usage_emit_nothing() {
    let expected = case("compaction_checks", "error_without_usage_does_nothing");
    on_paused_task_set(|_| async {
        let h = seeded(json!({}), vec![summary_provider()]).await;
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
        let h = seeded(json!({}), vec![summary_provider()]).await;
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
            vec![summary_provider()],
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
fn queued_messages_resume_after_the_compaction() {
    let expected = case(
        "compaction_checks",
        "queue_only_compaction_resumes_at_100ms",
    );
    on_paused_task_set(|_| async {
        let h = seeded(json!({}), vec![summary_provider()]).await;
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
        let ended = agent_ended(&h);
        let message = response(400, &answer_extra(&StopReason::Stop, None, 199_999));
        h.session.check_compaction(&message, true).await.unwrap();
        assert_observation(&h, &expected["before"]);
        tokio::time::advance(std::time::Duration::from_millis(99)).await;
        assert_eq!(messages_view(&h), expected["before"]["messages"]);
        drop(script_responses(
            &h,
            vec![answer("queued answer", None, None)],
        ));
        tokio::time::advance(std::time::Duration::from_millis(1)).await;
        ended.notified().await;
        assert_eq!(messages_view(&h), expected["after"]["messages"]);
    });
}

#[test]
fn automatic_compaction_without_credentials_ends_silently() {
    let expected = case(
        "compaction_checks",
        "auto_compaction_without_auth_ends_silently",
    );
    on_paused_task_set(|_| async {
        let h = unauthenticated(vec![summary_provider()]).await;
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
            factories: vec![summary_provider()],
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
        let h = seeded(json!({}), vec![summary_provider()]).await;
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
        let h = seeded(json!({}), vec![summary_provider()]).await;
        let result = h.session.compact(None).await.unwrap();
        assert_eq!(
            result.summary,
            expected["result"]["summary"].as_str().unwrap()
        );
        assert_eq!(
            recorded_number(result.tokens_before),
            expected["result"]["tokensBefore"]
        );
        assert_eq!(result.details.present().unwrap(), &json!({}));
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
        assert_eq!(result.summary, "generated summary");
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
        let h = seeded(json!({}), vec![summary_provider()]).await;
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
            factories: vec![summary_provider()],
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
            Err(error) => {
                let expected = expected["refused"]["error"]
                    .as_str()
                    .unwrap()
                    .replace("$DOCS", &docs);
                assert_eq!(error.message, expected);
            }
        }
        let docs = maestro_app::configuration::config::get_docs_path().unwrap();
        let observed = events_view(&h).to_string().replace(&docs, "$DOCS");
        assert_eq!(
            serde_json::from_str::<Value>(&observed).unwrap(),
            without_kept_identity(&expected["events"])
        );
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
        let h = seeded(json!({}), vec![summary_provider()]).await;
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
            h.session.handle_retryable_error(&failing).await.unwrap(),
            expected["afterFirst"]["returned"].as_bool().unwrap()
        );
        assert_observation(&h, &expected["afterFirst"]);
        assert_eq!(
            h.session.handle_retryable_error(&failing).await.unwrap(),
            expected["second"].as_bool().unwrap()
        );
        assert_eq!(
            h.session.handle_retryable_error(&failing).await.unwrap(),
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
            h.session.handle_retryable_error(&failing).await.unwrap(),
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
        assert_eq!(
            task.await.unwrap().unwrap(),
            expected["returned"].as_bool().unwrap()
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
fn a_retry_continues_immediately_after_the_backoff() {
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
        set_agent_messages(
            &h,
            &[
                user(&json!([{"type": "text", "text": "hello"}]), 100),
                serde_json::to_value(&failing).unwrap(),
            ],
        );
        let ended = agent_ended(&h);
        let mut work = Box::pin(h.session.handle_retryable_error(&failing));
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        assert!(std::future::Future::poll(work.as_mut(), &mut context).is_pending());
        tokio::time::advance(std::time::Duration::from_millis(1_999)).await;
        assert!(std::future::Future::poll(work.as_mut(), &mut context).is_pending());
        drop(script_responses(
            &h,
            vec![answer("retried answer", None, None)],
        ));
        tokio::time::advance(std::time::Duration::from_millis(1)).await;
        let std::task::Poll::Ready(returned) =
            std::future::Future::poll(work.as_mut(), &mut context)
        else {
            panic!("the backoff ends after 2000 ms");
        };
        assert_eq!(returned.unwrap(), expected["returned"].as_bool().unwrap());
        assert_eq!(events_view(&h), expected["events"]);
        ended.notified().await;
        assert_eq!(messages_view(&h), expected["messages"]);
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
            outcomes.push(h.session.handle_retryable_error(&failing).await.unwrap());
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
            h.session.handle_retryable_error(&failing).await.unwrap(),
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
            h.session.handle_retryable_error(&failing).await.unwrap(),
            expected["returned"].as_bool().unwrap()
        );
        assert_observation(&h, &expected);
    });
}

#[test]
fn waiting_for_a_retry_releases_on_abort() {
    let expected = case("retry_behaviour", "wait_for_retry_releases_on_abort");
    on_paused_task_set(|_| async {
        let h = harness(HarnessOptions {
            settings: json!({"retry": {"enabled": true, "maxRetries": 2, "baseDelayMs": 2000}}),
            ..HarnessOptions::default()
        })
        .await;
        let failing = response(
            400,
            &answer_extra(&StopReason::Error, Some("overloaded_error"), 0),
        );
        let started = retry_started(&h);
        let session = Rc::clone(&h.session);
        let retry =
            tokio::task::spawn_local(async move { session.handle_retryable_error(&failing).await });
        started.notified().await;
        let mut waiting = Box::pin(h.session.wait_for_retry());
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        let before = std::future::Future::poll(waiting.as_mut(), &mut context).is_ready();
        assert_eq!(before, expected["beforeRelease"].as_bool().unwrap());
        h.session.abort_retry();
        let after = std::future::Future::poll(waiting.as_mut(), &mut context).is_ready();
        assert_eq!(after, expected["after"].as_bool().unwrap());
        drop(retry.await);
    });
}

#[test]
fn toggles_persist_to_the_settings() {
    let expected = case("retry_behaviour", "toggles_write_settings");
    on_paused_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        h.session.set_auto_retry_enabled(false);
        h.session.set_auto_compaction_enabled(false);
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
        assert_eq!(
            task.await.unwrap().unwrap(),
            expected["returned"].as_bool().unwrap()
        );
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
            factories: vec![summary_provider()],
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
        let h = seeded(json!({}), vec![summary_provider()]).await;
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
        let h = seeded(json!({}), vec![summary_provider()]).await;
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
fn waiting_without_a_retry_returns_while_the_agent_runs() {
    on_paused_task_set(|_| async {
        let h = harness(HarnessOptions::default()).await;
        drop(script_responses(&h, vec![answer("busy", None, None)]));
        let run = h.session.agent().prompt(PromptInput::Text {
            text: "hello".to_owned(),
            images: Vec::new(),
        });
        let mut waiting = Box::pin(h.session.wait_for_retry());
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        assert!(std::future::Future::poll(waiting.as_mut(), &mut context).is_ready());
        run.await.unwrap();
    });
}

#[test]
fn repeated_summaries_announce_the_newest_entry() {
    on_paused_task_set(|_| async {
        let ids = Rc::new(RefCell::new(Vec::new()));
        let h = seeded(
            json!({}),
            vec![summary_provider(), announced(Rc::clone(&ids))],
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
fn dropping_a_waiting_retry_cancels_it() {
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
        drop_mid_flight(task).await;
        assert_observation(&h, &expected);
        assert_eq!(h.session.retry_attempt(), 0);
        assert!(!h.session.is_retrying());
    });
}

#[test]
fn dropping_a_manual_compaction_ends_it_as_aborted() {
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
        drop_mid_flight(task).await;
        assert_observation(&h, &expected);
        assert!(!h.session.is_compacting());
    });
}

#[test]
fn dropping_an_automatic_compaction_ends_it_as_aborted() {
    let expected = case(
        "compaction_checks",
        "auto_compaction_cancelled_by_extension",
    );
    on_paused_task_set(|_| async {
        let reached = Rc::new(Notify::new());
        let h = seeded(json!({}), vec![waiting_extension(Rc::clone(&reached))]).await;
        let overflow = overflow_response();
        push_agent_message(&h, &overflow);
        let session = Rc::clone(&h.session);
        let task =
            tokio::task::spawn_local(
                async move { session.check_compaction(&overflow, true).await },
            );
        reached.notified().await;
        drop_mid_flight(task).await;
        assert_observation(&h, &expected);
        assert!(!h.session.is_compacting());
    });
}

#[test]
fn dropping_a_tree_navigation_clears_its_branch_summary() {
    on_paused_task_set(|_| async {
        let reached = Rc::new(Notify::new());
        let h = seeded(json!({}), vec![waiting_tree_extension(Rc::clone(&reached))]).await;
        append_user(&h, "three", 400);
        let target = first_entry_id(&h);
        let session = Rc::clone(&h.session);
        let task = tokio::task::spawn_local(async move {
            session.navigate_tree(&target, navigate_options()).await
        });
        reached.notified().await;
        assert!(h.session.is_compacting());
        drop_mid_flight(task).await;
        assert!(!h.session.is_compacting());
    });
}
