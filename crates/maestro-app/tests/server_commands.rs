//! Each command over the protocol, answered by a real session runtime.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::rpc_connection::{Client, connect, of_type, replacement_watch, response_to};
use crate::support::runtime_fixtures::{RuntimeOptions, runtime_for, runtime_with, stored_in};
use crate::support::session_harness::{
    Auth, Harness, answer, hold_first_answer, input_extension, on_task_set, script_responses,
};
use maestro_app::{AgentSessionRuntime, RpcConnectionOutcome};
use maestro_extensions::{
    BeforeAgentStartEvent, BeforeAgentStartEventResult, ExtensionFactory, InputEventResult,
};
use maestro_models::Local;
use maestro_session::session_manager::ReadonlySessionManager;
use serde_json::{Value, json};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// `value` as a client reads it from the compact protocol text.
fn wire(value: &Value) -> Value {
    serde_json::from_str(&maestro_app::compact_json(value).unwrap()).unwrap()
}

/// The state reply carries every member of the live session, and the input's end
/// finishes the connection without exiting.
#[test]
fn state_preserves_omissions_and_live_counts() {
    on_task_set(|_| async {
        let (_h, runtime, _) = runtime_for("state", Vec::new()).await;
        let runtime = Rc::new(runtime);
        let session = runtime.session();
        session.follow_up("queued", &[]).unwrap();
        let mut client = connect(Rc::clone(&runtime));
        client.send(r#"{"id":"a","type":"get_state"}"#);
        let model = wire(&serde_json::to_value(&*session.model().read().unwrap()).unwrap());
        assert_eq!(
            client.next().await,
            json!({
                "id": "a",
                "type": "response",
                "command": "get_state",
                "success": true,
                "data": {
                    "model": model,
                    "thinkingLevel": session.thinking_level(),
                    "isStreaming": false,
                    "isCompacting": false,
                    "steeringMode": "one-at-a-time",
                    "followUpMode": "one-at-a-time",
                    "sessionFile": session.session_file().unwrap(),
                    "sessionId": session.session_id(),
                    "autoCompactionEnabled": true,
                    "messageCount": 0,
                    "pendingMessageCount": 1,
                },
            })
        );
        session.set_session_name("named").unwrap();
        assert_eq!(
            client.next().await,
            json!({"type": "session_info_changed", "name": "named"})
        );
        client.send(r#"{"type":"get_state"}"#);
        let state = client.next().await;
        assert_eq!(state.get("id"), None);
        assert_eq!(state["data"]["sessionName"], "named");
        client.end(None);
        assert_eq!(client.outcome().await.unwrap(), RpcConnectionOutcome::Eof);
    });
}

/// A null record settles one uncorrelated failure, and the next command is still served.
#[test]
fn invalid_commands_settle_and_keep_serving() {
    on_task_set(|_| async {
        let (_h, runtime, _) = runtime_for("invalid", Vec::new()).await;
        let mut client = connect(Rc::new(runtime));
        for (line, error) in [
            (
                "null",
                "invalid type: null, expected internally tagged enum RpcSessionCommand",
            ),
            (
                "7",
                "invalid type: integer `7`, expected internally tagged enum RpcSessionCommand",
            ),
            ("[]", "missing field `type`"),
            (r#"{"id":"u"}"#, "missing field `type`"),
            (
                r#"{"id":"u","type":5}"#,
                "invalid type: integer `5`, expected variant identifier",
            ),
        ] {
            client.send(line);
            client.send(r#"{"id":"next","type":"get_state"}"#);
            assert_eq!(
                client.next().await,
                json!({"type": "response", "success": false, "error": error}),
                "{line}"
            );
            let next = client.next().await;
            assert_eq!(
                (&next["id"], &next["success"]),
                (&json!("next"), &json!(true))
            );
        }
        client.end(None);
        assert_eq!(client.outcome().await.unwrap(), RpcConnectionOutcome::Eof);
    });
}

/// A runtime whose extensions record each session start and veto switches and forks while
/// the returned flag is set, with its connection.
async fn watched(label: &str) -> (Harness, Rc<AgentSessionRuntime>, Client, Watch) {
    let watch = Watch::default();
    let factory = replacement_watch(Rc::clone(&watch.starts), Rc::clone(&watch.veto));
    let (h, runtime, _) = runtime_for(label, vec![(None, factory)]).await;
    let runtime = Rc::new(runtime);
    let mut client = connect(Rc::clone(&runtime));
    // The initial binding starts the session before any command is answered.
    client.ask(r#"{"type":"get_state"}"#).await;
    assert_eq!(*watch.starts.borrow(), [json!("startup")]);
    (h, runtime, client, watch)
}

/// The session starts and the switch and fork veto of a watched runtime.
#[derive(Default)]
struct Watch {
    /// Each session start's reason.
    starts: Rc<RefCell<Vec<Value>>>,
    /// Whether switches and forks are cancelled.
    veto: Rc<Cell<bool>>,
}

/// Runs one scripted turn and checks that each of its agent events reaches the client once.
async fn one_turn_reaches_client_once(h: &Harness, client: &mut Client) {
    script_responses(h, vec![answer("after", None, None)]);
    client.send(r#"{"id":"turn","type":"prompt","message":"next"}"#);
    let records = client.until(|record| record["type"] == "agent_end").await;
    assert_eq!(of_type(&records, "agent_start").len(), 1);
    assert_eq!(of_type(&records, "turn_end").len(), 1);
}

/// A new session records a nonempty parent, binds the replacement once and keeps one event
/// stream; a vetoed one binds nothing.
#[test]
fn new_session_respects_parent_veto_and_rebind() {
    on_task_set(|_| async {
        let (h, runtime, mut client, watch) = watched("new").await;
        watch.veto.set(true);
        for line in [
            r#"{"id":"n","type":"new_session"}"#,
            r#"{"id":"n","type":"new_session","parentSession":""}"#,
            r#"{"id":"n","type":"new_session","parentSession":"/parent.jsonl"}"#,
        ] {
            let reply = client.ask(line).await;
            assert_eq!(reply["data"], json!({"cancelled": true}), "{line}");
        }
        assert_eq!(watch.starts.borrow().len(), 1, "a veto binds nothing");
        watch.veto.set(false);
        let reply = client
            .ask(r#"{"id":"n","type":"new_session","parentSession":"/parent.jsonl"}"#)
            .await;
        assert_eq!(
            reply,
            json!({"id": "n", "type": "response", "command": "new_session", "success": true,
                "data": {"cancelled": false}})
        );
        assert_eq!(*watch.starts.borrow(), [json!("startup"), json!("new")]);
        let header = runtime
            .session()
            .session_manager()
            .borrow()
            .get_header()
            .cloned();
        assert_eq!(header.unwrap()["parentSession"], "/parent.jsonl");
        client
            .ask(r#"{"type":"new_session","parentSession":""}"#)
            .await;
        let header = runtime
            .session()
            .session_manager()
            .borrow()
            .get_header()
            .cloned();
        assert_eq!(header.unwrap().get("parentSession"), None);
        one_turn_reaches_client_once(&h, &mut client).await;
    });
}

/// A switch resumes the selected file, binds it once and keeps one event stream; a vetoed
/// one keeps the current session.
#[test]
fn session_switch_respects_veto_and_rebind() {
    on_task_set(|_| async {
        let (h, runtime, mut client, watch) = watched("switch").await;
        let root = h.scratch.path().display().to_string();
        let stored = stored_in(&format!("{root}/switch"), &format!("{root}/stored"), &root);
        let path = stored.get_session_file().unwrap().to_owned();
        let line = json!({"id": "s", "type": "switch_session", "sessionPath": path}).to_string();
        watch.veto.set(true);
        let current = runtime.session().session_id();
        assert_eq!(client.ask(&line).await["data"], json!({"cancelled": true}));
        assert_eq!(runtime.session().session_id(), current);
        watch.veto.set(false);
        assert_eq!(client.ask(&line).await["data"], json!({"cancelled": false}));
        assert_eq!(
            runtime.session().session_file().as_deref(),
            Some(path.as_str())
        );
        assert_eq!(*watch.starts.borrow(), [json!("startup"), json!("resume")]);
        one_turn_reaches_client_once(&h, &mut client).await;
    });
}

/// A fork before a user message answers its text; a vetoed fork omits the text and binds
/// nothing; a successful one binds once and keeps one event stream.
#[test]
fn fork_keeps_selected_text_and_veto() {
    on_task_set(|_| async {
        let (h, runtime, mut client, watch) = watched("fork").await;
        let entry = user_entry(&runtime, "fork from here");
        let line = json!({"id": "f", "type": "fork", "entryId": entry}).to_string();
        watch.veto.set(true);
        assert_eq!(client.ask(&line).await["data"], json!({"cancelled": true}));
        assert_eq!(watch.starts.borrow().len(), 1);
        watch.veto.set(false);
        assert_eq!(
            client.ask(&line).await["data"],
            json!({"text": "fork from here", "cancelled": false})
        );
        assert_eq!(*watch.starts.borrow(), [json!("startup"), json!("fork")]);
        one_turn_reaches_client_once(&h, &mut client).await;
    });
}

/// A clone without a current entry is refused; otherwise it forks at the current leaf,
/// binds once and keeps one event stream, and a vetoed clone binds nothing.
#[test]
fn clone_uses_current_leaf_at_position() {
    on_task_set(|_| async {
        let (h, runtime, mut client, watch) = watched("clone").await;
        // Navigating before the first entry leaves no current entry.
        runtime
            .session()
            .session_manager()
            .borrow_mut()
            .reset_leaf();
        assert_eq!(
            client.ask(r#"{"id":"c","type":"clone"}"#).await,
            json!({"id": "c", "type": "response", "command": "clone", "success": false,
                "error": "Cannot clone session: no current entry selected"})
        );
        user_entry(&runtime, "kept at the leaf");
        let leaf_messages = runtime.session().get_user_messages_for_forking().len();
        watch.veto.set(true);
        assert_eq!(
            client.ask(r#"{"type":"clone"}"#).await["data"],
            json!({"cancelled": true})
        );
        watch.veto.set(false);
        assert_eq!(
            client.ask(r#"{"type":"clone"}"#).await["data"],
            json!({"cancelled": false})
        );
        assert_eq!(
            runtime.session().get_user_messages_for_forking().len(),
            leaf_messages,
            "a clone at the leaf keeps the leaf's message"
        );
        assert_eq!(*watch.starts.borrow(), [json!("startup"), json!("fork")]);
        one_turn_reaches_client_once(&h, &mut client).await;
    });
}

/// Appends a user message with `text` and its answer to the current session, which writes
/// the session file; returns the user message's entry.
fn user_entry(runtime: &AgentSessionRuntime, text: &str) -> String {
    let session = runtime.session();
    let mut manager = session.session_manager().borrow_mut();
    let entry = manager
        .append_message(json!({"role": "user", "content": text, "timestamp": 1}))
        .unwrap();
    manager
        .append_message(serde_json::to_value(answer("answered", None, None)).unwrap())
        .unwrap();
    entry
}

/// A prompt is answered once, when its preflight succeeds, also when an input handler
/// handles it without a turn.
#[test]
fn prompt_preflight_acceptance_replies_once() {
    on_task_set(|_| async {
        let (h, runtime, _) = runtime_for("accepted", Vec::new()).await;
        script_responses(&h, vec![answer("done", None, None)]);
        let mut client = connect(Rc::new(runtime));
        client.send(r#"{"id":"b2","type":"prompt","message":"Hello"}"#);
        let records = client.until(|record| record["type"] == "agent_end").await;
        let replies: Vec<&Value> = records
            .iter()
            .filter(|record| response_to("prompt")(record))
            .collect();
        assert_eq!(
            replies,
            [&json!({"id": "b2", "type": "response", "command": "prompt", "success": true})]
        );
        let handles = input_extension(|_| Ok(Some(InputEventResult::Handled)));
        let (_h, runtime, _) = runtime_for("handled", vec![(None, handles)]).await;
        let mut client = connect(Rc::new(runtime));
        client.send(r#"{"id":"h","type":"prompt","message":"handled"}"#);
        assert_eq!(client.next().await["success"], true);
        let state = client.ask(r#"{"id":"after","type":"get_state"}"#).await;
        assert_eq!(
            state["id"], "after",
            "no turn and no second reply came first"
        );
    });
}

/// Without credentials the prompt fails once, before acceptance.
#[test]
fn prompt_preflight_rejection_replies_once() {
    on_task_set(|_| async {
        let options = RuntimeOptions {
            auth: Auth::None,
            ..RuntimeOptions::default()
        };
        let (_h, runtime, _) = runtime_with("refused", options).await;
        let mut client = connect(Rc::new(runtime));
        client.send(r#"{"id":"b1","type":"prompt","message":"Hello"}"#);
        let reply = client.next().await;
        assert_eq!(
            (&reply["id"], &reply["success"]),
            (&json!("b1"), &json!(false))
        );
        let error = reply["error"].as_str().unwrap();
        assert!(
            error.starts_with("No API key found for faux.\n\nUse /login"),
            "{error}"
        );
        let state = client.ask(r#"{"id":"after","type":"get_state"}"#).await;
        assert_eq!(state["id"], "after", "no second prompt reply came first");
    });
}

/// A prompt queued as a follow-up while a turn streams is answered once, before the
/// streaming turn ends.
#[test]
fn prompt_streaming_follow_up_replies_once() {
    on_task_set(|_| async {
        let (h, runtime, _) = runtime_for("queued", Vec::new()).await;
        let (requested, release) = hold_first_answer(&h, "first", &["second"]);
        let mut client = connect(Rc::new(runtime));
        client.send(r#"{"id":"start","type":"prompt","message":"Start"}"#);
        client.until(|record| record["id"] == "start").await;
        requested.notified().await;
        client.send(
            r#"{"id":"next","type":"prompt","message":"Next","streamingBehavior":"followUp"}"#,
        );
        let before = client.until(|record| record["id"] == "next").await;
        assert_eq!(before.last().unwrap()["success"], true);
        assert!(of_type(&before, "agent_end").is_empty(), "still streaming");
        release.notify_one();
        let after = client.until(|record| record["type"] == "agent_end").await;
        assert!(after.iter().all(|record| record["id"] != "next"));
    });
}

/// A failure after the prompt was accepted is not reported.
#[test]
fn prompt_post_acceptance_error_stays_silent() {
    on_task_set(|_| async {
        let gates: Rc<Gates> = Rc::default();
        let (h, runtime, _) = runtime_for("late", vec![(None, gated_start(&gates))]).await;
        let (requested, release) = hold_first_answer(&h, "first", &[]);
        let mut client = connect(Rc::new(runtime));
        client.send(r#"{"id":"one","type":"prompt","message":"one"}"#);
        client.send(r#"{"id":"two","type":"prompt","message":"two"}"#);
        let (first, second) = (gates.wait_held().await, gates.wait_held().await);
        first.send(()).unwrap();
        client.until(|record| record["id"] == "one").await;
        requested.notified().await;
        second.send(()).unwrap();
        let accepted = client.until(|record| record["id"] == "two").await;
        assert_eq!(accepted.last().unwrap()["success"], true);
        release.notify_one();
        let rest = client.until(|record| record["type"] == "agent_end").await;
        assert!(rest.iter().all(|record| record["id"] != "two"), "{rest:?}");
        let state = client.ask(r#"{"id":"after","type":"get_state"}"#).await;
        assert_eq!(state["id"], "after");
    });
}

/// Before-agent-start handlers waiting one at a time for the test to open them.
#[derive(Default)]
struct Gates {
    /// Openers of the waiting handlers, in arrival order.
    held: RefCell<std::collections::VecDeque<tokio::sync::oneshot::Sender<()>>>,
    /// Signals each arrival.
    arrived: tokio::sync::Notify,
}

impl Gates {
    /// The opener of the next waiting handler, once one waits.
    async fn wait_held(&self) -> tokio::sync::oneshot::Sender<()> {
        loop {
            let arrived = self.arrived.notified();
            if let Some(opener) = self.held.borrow_mut().pop_front() {
                return opener;
            }
            arrived.await;
        }
    }
}

/// An extension whose before-agent-start handler waits for its gate on every prompt.
fn gated_start(gates: &Rc<Gates>) -> ExtensionFactory<Local> {
    let gates = Rc::clone(gates);
    Rc::new(move |api| {
        let gates = Rc::clone(&gates);
        let registered = api.on::<BeforeAgentStartEvent, BeforeAgentStartEventResult>(
            "before_agent_start".into(),
            Rc::new(move |_, _| {
                let (opener, gate) = tokio::sync::oneshot::channel();
                gates.held.borrow_mut().push_back(opener);
                gates.arrived.notify_one();
                Box::pin(async move {
                    drop(gate.await);
                    Ok(None)
                })
            }),
        );
        Box::pin(std::future::ready(registered.map(drop)))
    })
}

/// The prompt reaches input handlers as `rpc` with its images, and a prompt during a turn
/// is queued as the supplied delivery.
#[test]
fn prompt_keeps_source_images_and_queue_choice() {
    on_task_set(|_| async {
        let seen = Rc::new(RefCell::new(Vec::new()));
        let record = Rc::clone(&seen);
        let watch = input_extension(move |event| {
            record
                .borrow_mut()
                .push(serde_json::to_value(event).unwrap());
            Ok(None)
        });
        let (h, runtime, _) = runtime_for("sourced", vec![(None, watch)]).await;
        let runtime = Rc::new(runtime);
        let (requested, release) = hold_first_answer(&h, "first", &["second", "third"]);
        let mut client = connect(Rc::clone(&runtime));
        let image = json!({"type": "image", "data": "AAAA", "mimeType": "image/png"});
        let prompt = json!({"id": "p", "type": "prompt", "message": "look", "images": [image]});
        client.send(&prompt.to_string());
        client.until(|record| record["id"] == "p").await;
        assert_eq!(
            *seen.borrow(),
            [json!({"text": "look", "images": [image], "source": "rpc"})]
        );
        requested.notified().await;
        for (id, behavior) in [("s", "steer"), ("f", "followUp")] {
            let queued = json!({"id": id, "type": "prompt", "message": id,
                "streamingBehavior": behavior});
            client.send(&queued.to_string());
            client.until(|record| record["id"] == id).await;
        }
        let session = runtime.session();
        assert_eq!(session.get_steering_messages(), ["s"]);
        assert_eq!(session.get_follow_up_messages(), ["f"]);
        release.notify_one();
        client.until(|record| record["type"] == "agent_end").await;
    });
}
