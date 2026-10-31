//! Each command over the protocol, answered by a real session runtime.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::rpc_connection::{
    Client, connect, of_type, replacement_watch, response_to, user_entry,
};
use crate::support::runtime_fixtures::{
    Recorded, Resources, RuntimeOptions, factory, runtime_for, runtime_with, stored_in,
};
use crate::support::session_harness::{
    Auth, Harness, HarnessOptions, ScriptedShell, answer, assistant, command_extension,
    create_test_extensions_result, harness, hold_first_answer, input_extension, on_task_set,
    script_responses, template, usage,
};
use maestro_app::agent_session::BashRunOptions;
use maestro_app::{
    AgentSessionRuntime, CreateAgentSessionRuntimeOptions, RpcConnectionOutcome,
    create_agent_session_runtime,
};
use maestro_extensions::{
    BeforeAgentStartEvent, BeforeAgentStartEventResult, ExtensionFactory, InputEventResult,
};
use maestro_models::Local;
use maestro_session::SessionManager;
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

/// A runtime with its connection, after the initial binding.
async fn served(label: &str) -> (Harness, Rc<AgentSessionRuntime>, Client) {
    let (h, runtime, _) = runtime_for(label, Vec::new()).await;
    let runtime = Rc::new(runtime);
    let client = connect(Rc::clone(&runtime));
    (h, runtime, client)
}

/// The image every image-carrying case sends.
fn image() -> Value {
    json!({"type": "image", "data": "AAAA", "mimeType": "image/png"})
}

/// Queues `message` with an image through `command` while a turn streams, and returns the
/// user message the agent later delivered with it.
async fn queued_message_delivered(command: &str) -> Value {
    let (h, _runtime, mut client) = served(command).await;
    let (requested, release) = hold_first_answer(&h, "first", &["second"]);
    client.send(r#"{"id":"start","type":"prompt","message":"Start"}"#);
    client.until(|record| record["id"] == "start").await;
    requested.notified().await;
    let line = json!({"id": "q", "type": command, "message": "look", "images": [image()]});
    let reply = client.ask(&line.to_string()).await;
    assert_eq!(
        reply,
        json!({"id": "q", "type": "response", "command": command, "success": true})
    );
    release.notify_one();
    let records = client.until(|record| record["type"] == "agent_end").await;
    let delivered = records.iter().find(|record| {
        record["type"] == "message_start"
            && record["message"]["role"] == "user"
            && record["message"]["content"][0]["text"] == "look"
    });
    delivered.unwrap()["message"]["content"].clone()
}

/// A steering message keeps its text and images until the agent delivers it.
#[test]
fn steering_keeps_supplied_images() {
    on_task_set(|_| async {
        let content = queued_message_delivered("steer").await;
        assert_eq!(content, json!([{"type": "text", "text": "look"}, image()]));
    });
}

/// A follow-up message keeps its text and images until the agent delivers it.
#[test]
fn follow_up_keeps_supplied_images() {
    on_task_set(|_| async {
        let content = queued_message_delivered("follow_up").await;
        assert_eq!(content, json!([{"type": "text", "text": "look"}, image()]));
    });
}

/// Abort answers only once the agent is idle.
#[test]
fn abort_reply_follows_idle() {
    on_task_set(|_| async {
        let (h, runtime, mut client) = served("abort").await;
        let (requested, release) = hold_first_answer(&h, "first", &[]);
        client.send(r#"{"id":"start","type":"prompt","message":"Start"}"#);
        client.until(|record| record["id"] == "start").await;
        requested.notified().await;
        client.send(r#"{"id":"a","type":"abort"}"#);
        let waiting = client.ask(r#"{"id":"s","type":"get_state"}"#).await;
        assert_eq!(waiting["id"], "s", "abort waits for the streaming turn");
        assert_eq!(waiting["data"]["isStreaming"], true);
        release.notify_one();
        let reply = client.until(response_to("abort")).await.pop().unwrap();
        assert!(!runtime.session().is_streaming());
        assert_eq!(
            reply,
            json!({"id": "a", "type": "response", "command": "abort", "success": true})
        );
    });
}

/// Options with three scripted models.
fn three_models() -> RuntimeOptions {
    RuntimeOptions {
        models: vec![("faux-1", true), ("plain", false), ("think", true)],
        ..RuntimeOptions::default()
    }
}

/// `set_model` selects the available model with the exact provider and identifier and
/// answers with it; any other pair is not found.
#[test]
fn set_model_uses_exact_available_pair() {
    on_task_set(|_| async {
        let (h, runtime, _) = runtime_with("model", three_models()).await;
        let runtime = Rc::new(runtime);
        let mut client = connect(Rc::clone(&runtime));
        let reply = client
            .ask(r#"{"id":"m","type":"set_model","provider":"faux","modelId":"think"}"#)
            .await;
        let selected = runtime.session().model();
        assert_eq!(selected.read().unwrap().id, "think");
        let model = wire(&serde_json::to_value(&*selected.read().unwrap()).unwrap());
        assert_eq!(
            reply,
            json!({"id": "m", "type": "response", "command": "set_model", "success": true,
                "data": model})
        );
        for (provider, id) in [("faux", "nope"), ("elsewhere", "think"), ("faux", "Think")] {
            let line = json!({"id": "x", "type": "set_model", "provider": provider, "modelId": id});
            assert_eq!(
                client.ask(&line.to_string()).await,
                json!({"id": "x", "type": "response", "command": "set_model", "success": false,
                    "error": format!("Model not found: {provider}/{id}")})
            );
        }
        assert_eq!(runtime.session().model().read().unwrap().id, "think");
        drop(h);
    });
}

/// Cycling with one model answers `null`; with several, the selected model, its thinking
/// level and whether the scoped models were cycled.
#[test]
fn model_cycle_keeps_scope_and_thinking() {
    on_task_set(|_| async {
        let (_h, _runtime, mut client) = served("single").await;
        assert_eq!(
            client.ask(r#"{"type":"cycle_model"}"#).await["data"],
            Value::Null
        );
        let (_h, runtime, _) = runtime_with("cycled", three_models()).await;
        let runtime = Rc::new(runtime);
        let mut client = connect(Rc::clone(&runtime));
        let reply = client.ask(r#"{"type":"cycle_model"}"#).await;
        let session = runtime.session();
        let model = wire(&serde_json::to_value(&*session.model().read().unwrap()).unwrap());
        assert_eq!(
            reply["data"],
            json!({"model": model, "thinkingLevel": session.thinking_level(), "isScoped": false})
        );
    });
}

/// The available models are answered in full and in catalog order, also when there are
/// none.
#[test]
fn available_models_keep_full_descriptors() {
    on_task_set(|_| async {
        let (_h, runtime, _) = runtime_with("available", three_models()).await;
        let runtime = Rc::new(runtime);
        let mut client = connect(Rc::clone(&runtime));
        let available = runtime.session().model_registry().get_available().unwrap();
        let models: Vec<Value> = available
            .iter()
            .map(|model| wire(&serde_json::to_value(&*model.read().unwrap()).unwrap()))
            .collect();
        assert_eq!(models.len(), 3);
        let reply = client.ask(r#"{"type":"get_available_models"}"#).await;
        assert_eq!(reply["data"], json!({"models": models}));
        let options = RuntimeOptions {
            auth: Auth::None,
            ..RuntimeOptions::default()
        };
        let (_h, runtime, _) = runtime_with("unavailable", options).await;
        let mut client = connect(Rc::new(runtime));
        let reply = client.ask(r#"{"type":"get_available_models"}"#).await;
        assert_eq!(reply["data"], json!({"models": []}));
    });
}

/// A thinking level goes through the session's clamping and announcement.
#[test]
fn thinking_set_uses_session_policy() {
    on_task_set(|_| async {
        let (_h, runtime, mut client) = served("thinking").await;
        client.send(r#"{"id":"t","type":"set_thinking_level","level":"high"}"#);
        assert_eq!(
            client.next().await,
            json!({"type": "thinking_level_changed", "level": "high"})
        );
        assert_eq!(
            client.next().await,
            json!({"id": "t", "type": "response", "command": "set_thinking_level",
                "success": true})
        );
        assert_eq!(
            runtime.session().thinking_level(),
            maestro_models::ModelThinkingLevel::High
        );
        let options = RuntimeOptions {
            models: vec![("faux-1", false)],
            ..RuntimeOptions::default()
        };
        let (_h, runtime, _) = runtime_with("plain", options).await;
        let runtime = Rc::new(runtime);
        let mut client = connect(Rc::clone(&runtime));
        let reply = client
            .ask(r#"{"type":"set_thinking_level","level":"high"}"#)
            .await;
        assert_eq!(reply["success"], true);
        assert_eq!(
            runtime.session().thinking_level(),
            maestro_models::ModelThinkingLevel::Off,
            "a model without reasoning clamps to off"
        );
    });
}

/// Cycling the thinking level answers the next level, or `null` without reasoning.
#[test]
fn thinking_cycle_distinguishes_null_and_level() {
    on_task_set(|_| async {
        let (_h, runtime, mut client) = served("cycle").await;
        let reply = client.ask(r#"{"type":"cycle_thinking_level"}"#).await;
        let level = runtime.session().thinking_level();
        assert_eq!(reply["data"], json!({"level": level}));
        let options = RuntimeOptions {
            models: vec![("faux-1", false)],
            ..RuntimeOptions::default()
        };
        let (_h, runtime, _) = runtime_with("flat", options).await;
        let mut client = connect(Rc::new(runtime));
        let reply = client.ask(r#"{"type":"cycle_thinking_level"}"#).await;
        assert_eq!(
            reply,
            json!({"type": "response", "command": "cycle_thinking_level", "success": true,
                "data": null})
        );
    });
}

/// Each queue mode command sets its own mode, both directions, and leaves the other one.
async fn queue_modes_stay_independent(set: &str, other: &str) {
    let (_h, _runtime, mut client) = served(set).await;
    for (mode, other_mode) in [("all", "one-at-a-time"), ("one-at-a-time", "all")] {
        let line = json!({"type": format!("set_{other}_mode"), "mode": other_mode});
        assert_eq!(client.ask(&line.to_string()).await["success"], true);
        let line = json!({"id": "m", "type": format!("set_{set}_mode"), "mode": mode});
        assert_eq!(
            client.ask(&line.to_string()).await,
            json!({"id": "m", "type": "response", "command": format!("set_{set}_mode"),
                "success": true})
        );
        let state = client.ask(r#"{"type":"get_state"}"#).await;
        let key = |name: &str| {
            if name == "steering" {
                "steeringMode"
            } else {
                "followUpMode"
            }
        };
        assert_eq!(state["data"][key(set)], mode);
        assert_eq!(state["data"][key(other)], other_mode);
    }
}

/// The steering mode is set without changing the follow-up mode.
#[test]
fn steering_mode_is_independent_of_follow_up() {
    on_task_set(|_| queue_modes_stay_independent("steering", "follow_up"));
}

/// The follow-up mode is set without changing the steering mode.
#[test]
fn follow_up_mode_is_independent_of_steering() {
    on_task_set(|_| queue_modes_stay_independent("follow_up", "steering"));
}

/// Automatic compaction is switched off and on through the session.
#[test]
fn auto_compaction_forwards_enabled() {
    on_task_set(|_| async {
        let (_h, runtime, mut client) = served("compaction").await;
        for enabled in [false, true] {
            let line = json!({"id": "c", "type": "set_auto_compaction", "enabled": enabled});
            assert_eq!(
                client.ask(&line.to_string()).await,
                json!({"id": "c", "type": "response", "command": "set_auto_compaction",
                    "success": true})
            );
            assert_eq!(runtime.session().auto_compaction_enabled(), enabled);
        }
    });
}

/// Automatic retry is switched off and on through the session.
#[test]
fn auto_retry_forwards_enabled() {
    on_task_set(|_| async {
        let (_h, runtime, mut client) = served("retry").await;
        for enabled in [false, true] {
            let line = json!({"id": "r", "type": "set_auto_retry", "enabled": enabled});
            assert_eq!(
                client.ask(&line.to_string()).await,
                json!({"id": "r", "type": "response", "command": "set_auto_retry",
                    "success": true})
            );
            assert_eq!(runtime.session().auto_retry_enabled(), enabled);
        }
    });
}

/// Stopping a retry answers without data and leaves no retry running.
#[test]
fn abort_retry_keeps_session_ownership() {
    on_task_set(|_| async {
        let (_h, runtime, mut client) = served("abort-retry").await;
        assert_eq!(
            client.ask(r#"{"id":"r","type":"abort_retry"}"#).await,
            json!({"id": "r", "type": "response", "command": "abort_retry", "success": true})
        );
        assert!(!runtime.session().is_retrying());
    });
}

/// A shell command's nonzero status is still a successful reply carrying its output.
#[test]
fn bash_nonzero_status_is_success_data() {
    on_task_set(|_| async {
        let (_h, _runtime, mut client) = served("bash").await;
        let reply = client
            .ask(r#"{"id":"b","type":"bash","command":"printf output; exit 7"}"#)
            .await;
        assert_eq!(
            reply,
            json!({"id": "b", "type": "response", "command": "bash", "success": true,
                "data": {"output": "output", "exitCode": 7, "cancelled": false,
                    "truncated": false}})
        );
    });
}

/// Stopping the shell cancels the running command and answers without data.
#[test]
fn abort_bash_reaches_running_command() {
    on_task_set(|_| async {
        let (_h, _runtime, mut client) = served("abort-bash").await;
        client.send(r#"{"id":"b","type":"bash","command":"sleep 30"}"#);
        client.send(r#"{"id":"a","type":"abort_bash"}"#);
        let records = client.until(response_to("bash")).await;
        assert!(records.contains(
            &json!({"id": "a", "type": "response", "command": "abort_bash", "success": true})
        ));
        let bash = records.last().unwrap();
        assert_eq!(bash["data"]["cancelled"], true);
    });
}

/// Statistics carry every counter, the session file and the context usage.
#[test]
fn stats_keep_optional_usage() {
    on_task_set(|_| async {
        let (h, runtime, mut client) = served("stats").await;
        script_responses(&h, vec![answer("first", None, None)]);
        client.send(r#"{"type":"prompt","message":"hello"}"#);
        client.until(|record| record["type"] == "agent_end").await;
        let reply = client.ask(r#"{"id":"s","type":"get_session_stats"}"#).await;
        let stats = runtime.session().get_session_stats().unwrap();
        assert_eq!(reply["data"], wire(&serde_json::to_value(&stats).unwrap()));
        assert_eq!(reply["data"]["sessionFile"], json!(stats.session_file));
        assert_eq!(reply["data"]["userMessages"], 1);
        assert!(reply["data"]["contextUsage"]["contextWindow"].is_number());
    });
}

/// An export writes to the supplied path, or under a default name, and a failing write is
/// a correlated failure.
#[test]
fn html_export_keeps_supplied_path() {
    on_task_set(|_| async {
        let (h, runtime, mut client) = served("export").await;
        user_entry(&runtime, "exported");
        let root = h.scratch.path().display().to_string();
        let output = format!("{root}/session.html");
        let line = json!({"id": "e", "type": "export_html", "outputPath": output});
        assert_eq!(
            client.ask(&line.to_string()).await,
            json!({"id": "e", "type": "response", "command": "export_html", "success": true,
                "data": {"path": output}})
        );
        assert!(std::path::Path::new(&output).is_file());
        let omitted = client.ask(r#"{"type":"export_html"}"#).await;
        let default = omitted["data"]["path"].as_str().unwrap().to_owned();
        // The default name is relative to the process directory.
        std::fs::remove_file(&default).unwrap();
        let file = runtime.session().session_file().unwrap();
        let name = std::path::Path::new(&file)
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap();
        assert_eq!(default, format!("maestro-session-{name}.html"));
        let line = json!({"id": "d", "type": "export_html", "outputPath": root});
        let failed = client.ask(&line.to_string()).await;
        assert_eq!(
            (&failed["id"], &failed["success"]),
            (&json!("d"), &json!(false))
        );
        assert!(
            failed["error"]
                .as_str()
                .is_some_and(|error| !error.is_empty())
        );
    });
}

/// Every user message the session can fork from is listed in order, also when there is
/// none.
#[test]
fn fork_messages_keep_all_user_entries() {
    on_task_set(|_| async {
        let (_h, runtime, mut client) = served("forkable").await;
        assert_eq!(
            client.ask(r#"{"type":"get_fork_messages"}"#).await["data"],
            json!({"messages": []})
        );
        let first = user_entry(&runtime, "first");
        let second = user_entry(&runtime, "second");
        assert_eq!(
            client.ask(r#"{"type":"get_fork_messages"}"#).await["data"],
            json!({"messages": [{"entryId": first, "text": "first"},
                {"entryId": second, "text": "second"}]})
        );
    });
}

/// Without an assistant message the text is omitted; otherwise it is the trimmed text.
#[test]
fn last_assistant_text_omits_absent_text() {
    on_task_set(|_| async {
        let (h, _runtime, mut client) = served("last").await;
        assert_eq!(
            client
                .ask(r#"{"id":"l","type":"get_last_assistant_text"}"#)
                .await,
            json!({"id": "l", "type": "response", "command": "get_last_assistant_text",
                "success": true, "data": {}})
        );
        script_responses(&h, vec![answer(" answer ", None, None)]);
        client.send(r#"{"type":"prompt","message":"hello"}"#);
        client.until(|record| record["type"] == "agent_end").await;
        assert_eq!(
            client.ask(r#"{"type":"get_last_assistant_text"}"#).await["data"],
            json!({"text": "answer"})
        );
    });
}

/// Session names are trimmed of the shared whitespace set, U+FEFF included and U+0085 and
/// U+200B kept, and an empty name is refused.
#[test]
fn session_name_uses_exact_whitespace_set() {
    on_task_set(|_| async {
        let (_h, runtime, mut client) = served("names").await;
        let line = json!({"id": "n", "type": "set_session_name", "name": "\u{feff} \t\r\n"});
        assert_eq!(
            client.ask(&line.to_string()).await,
            json!({"id": "n", "type": "response", "command": "set_session_name",
                "success": false, "error": "Session name cannot be empty"})
        );
        for (supplied, stored) in [
            ("\u{feff} name\u{feff}", "name"),
            ("\u{85}name\u{85}", "\u{85}name\u{85}"),
            ("\u{200b}name ", "\u{200b}name"),
        ] {
            let line = json!({"type": "set_session_name", "name": supplied});
            assert_eq!(client.ask(&line.to_string()).await["success"], true);
            assert_eq!(
                runtime.session().session_name().unwrap().as_deref(),
                Some(stored)
            );
        }
    });
}

/// Messages are answered with every role and member in session order.
#[test]
fn messages_keep_roles_details_and_order() {
    on_task_set(|_| async {
        let (h, runtime, mut client) = served("messages").await;
        assert_eq!(
            client.ask(r#"{"type":"get_messages"}"#).await["data"],
            json!({"messages": []})
        );
        script_responses(&h, vec![answer("first", None, None)]);
        client.send(r#"{"type":"prompt","message":"hello"}"#);
        client.until(|record| record["type"] == "agent_end").await;
        let reply = client.ask(r#"{"type":"get_messages"}"#).await;
        let messages = runtime.session().messages();
        let expected = wire(&serde_json::to_value(&*messages.read().unwrap()).unwrap());
        assert_eq!(reply["data"], json!({"messages": expected}));
        let roles: Vec<&Value> = expected
            .as_array()
            .unwrap()
            .iter()
            .map(|message| &message["role"])
            .collect();
        assert_eq!(roles, [&json!("user"), &json!("assistant")]);
    });
}

/// A runtime over a stored history holding one answer without a reported total.
async fn runtime_over_saved_history(h: &Harness) -> Rc<AgentSessionRuntime> {
    let root = h.scratch.path().display().to_string();
    let cwd = format!("{root}/history");
    std::fs::create_dir_all(&cwd).unwrap();
    let file = {
        let mut manager =
            SessionManager::create(&cwd, Some(&format!("{cwd}/sessions")), &root).unwrap();
        manager
            .append_message(json!({"role": "user", "content": "saved", "timestamp": 0}))
            .unwrap();
        manager
            .append_message(assistant(
                "saved answer",
                0,
                &json!({"usage": usage([7, 3, 2, 1], 1.0, None)}),
            ))
            .unwrap();
        manager
            .get_session_file()
            .expect("a stored session file")
            .to_owned()
    };
    let manager = SessionManager::open(&file, None, None).unwrap();
    let recorded = Recorded::default();
    let extensions = Rc::new(create_test_extensions_result(Vec::new(), &cwd).await);
    Rc::new(
        create_agent_session_runtime(
            factory(h, &Resources::of(&extensions), &recorded),
            CreateAgentSessionRuntimeOptions {
                cwd,
                agent_dir: root,
                session_manager: Rc::new(RefCell::new(manager)),
                session_start_event: None,
            },
        )
        .await
        .expect("the stored session opens"),
    )
}

/// Messages restored from a history without a reported total keep that member absent.
#[test]
fn rpc_history_keeps_missing_total_omitted() {
    on_task_set(|_| async {
        let h = harness(HarnessOptions {
            models: vec![("faux-1", true)],
            ..HarnessOptions::default()
        })
        .await;
        let mut client = connect(runtime_over_saved_history(&h).await);
        let reply = client.ask(r#"{"id":"m","type":"get_messages"}"#).await;
        assert_eq!(reply["success"], true, "{reply}");
        let messages = reply["data"]["messages"].as_array().expect("messages");
        assert_eq!(messages.len(), 2);
        let saved = &messages[1];
        assert_eq!(saved["role"], "assistant");
        assert_eq!(saved["usage"].get("totalTokens"), None, "{saved}");
        for (member, expected) in [
            (&saved["usage"]["input"], 7.0),
            (&saved["usage"]["output"], 3.0),
            (&saved["usage"]["cacheRead"], 2.0),
            (&saved["usage"]["cacheWrite"], 1.0),
            (&saved["usage"]["cost"]["total"], 1.0),
        ] {
            assert_eq!(member.as_f64(), Some(expected), "{member}");
        }
        client.end(None);
        assert_eq!(client.outcome().await.unwrap(), RpcConnectionOutcome::Eof);
    });
}

/// Invocable commands list extension commands, then prompt templates, then skills, each
/// with its provenance.
#[test]
fn commands_keep_order_and_provenance() {
    on_task_set(|_| async {
        let heard = Rc::new(RefCell::new(None));
        let options = RuntimeOptions {
            factories: vec![(None, command_extension("deploy", heard, None))],
            prompts: vec![template("plan", "Plan it")],
            ..RuntimeOptions::default()
        };
        let (h, runtime, _) = runtime_with("commands", options).await;
        let runtime = Rc::new(runtime);
        let mut client = connect(Rc::clone(&runtime));
        let reply = client.ask(r#"{"id":"c","type":"get_commands"}"#).await;
        let commands = reply["data"]["commands"].as_array().unwrap();
        let names: Vec<&Value> = commands.iter().map(|command| &command["name"]).collect();
        assert_eq!(names, [&json!("deploy"), &json!("plan")]);
        assert_eq!(commands[0]["source"], "extension");
        assert_eq!(commands[1]["source"], "prompt");
        assert_eq!(commands[1]["description"], "");
        for command in commands {
            let info = command["sourceInfo"].as_object().unwrap();
            assert!(info.contains_key("path") && info.contains_key("source"));
            assert!(!command.as_object().unwrap().contains_key("location"));
        }
        drop(h);
    });
}

/// A long operation does not hold back later commands, and each reply keeps its id.
#[test]
fn responses_can_finish_out_of_order() {
    on_task_set(|_| async {
        let (_h, _runtime, mut client) = served("overlap").await;
        client.send(r#"{"id":"slow","type":"bash","command":"sleep 30"}"#);
        let state = client.ask(r#"{"id":"fast","type":"get_state"}"#).await;
        assert_eq!(state["id"], "fast");
        client.send(r#"{"id":"stop","type":"abort_bash"}"#);
        let records = client.until(response_to("bash")).await;
        let ids: Vec<&Value> = records.iter().map(|record| &record["id"]).collect();
        assert_eq!(ids, [&json!("stop"), &json!("slow")]);
    });
}

/// A recognized command with members of the wrong kind keeps its id and type and answers
/// the decoder's failure; a wrong-kind id is dropped.
#[test]
fn recognized_invalid_fields_keep_correlation() {
    on_task_set(|_| async {
        let (_h, _runtime, mut client) = served("fields").await;
        for (line, expected) in [
            (
                r#"{"id":"a","type":"set_model","provider":5,"modelId":"m"}"#,
                json!({"id": "a", "type": "response", "command": "set_model", "success": false,
                    "error": "invalid type: integer `5`, expected a string"}),
            ),
            (
                r#"{"id":"b","type":"prompt"}"#,
                json!({"id": "b", "type": "response", "command": "prompt", "success": false,
                    "error": "missing field `message`"}),
            ),
            (
                r#"{"id":"c","type":"set_steering_mode","mode":"some"}"#,
                json!({"id": "c", "type": "response", "command": "set_steering_mode",
                    "success": false,
                    "error": "unknown variant `some`, expected `all` or `one-at-a-time`"}),
            ),
            (
                // A null image list is no images.
                r#"{"id":"d","type":"prompt","message":"m","images":null}"#,
                json!({"id": "d", "type": "response", "command": "prompt", "success": true}),
            ),
            (
                r#"{"id":5,"type":"get_state"}"#,
                json!({"type": "response", "command": "get_state", "success": false,
                    "error": "invalid type: integer `5`, expected a string"}),
            ),
            (
                r#"{"id":"w","type":"workspace.get"}"#,
                json!({"id": "w", "type": "response", "command": "workspace.get",
                    "success": false, "error": "missing field `workspace_id`"}),
            ),
        ] {
            assert_eq!(client.ask(line).await, expected, "{line}");
        }
        let state = client.ask(r#"{"id":null,"type":"get_state"}"#).await;
        assert_eq!((state.get("id"), &state["success"]), (None, &json!(true)));
    });
}

/// A record that is not JSON answers the parse failure with the decoder's message and no
/// id, and the next command is served.
#[test]
fn malformed_json_has_native_uncorrelated_failure() {
    on_task_set(|_| async {
        let (_h, _runtime, mut client) = served("malformed").await;
        for (line, error) in [
            ("", "EOF while parsing a value at line 1 column 0"),
            ("   ", "EOF while parsing a value at line 1 column 3"),
            (
                r#"{"id":"x","type":"get_state""#,
                "EOF while parsing an object at line 1 column 28",
            ),
            (
                r#"{"id":"\ud800"}"#,
                "unexpected end of hex escape at line 1 column 14",
            ),
            ("01", "invalid number at line 1 column 2"),
            (
                &"[".repeat(200),
                "recursion limit exceeded at line 1 column 128",
            ),
        ] {
            client.send(line);
            assert_eq!(
                client.next().await,
                json!({"type": "response", "command": "parse", "success": false,
                    "error": format!("Failed to parse command: {error}")}),
                "{line}"
            );
        }
        let state = client.ask(r#"{"id":"next","type":"get_state"}"#).await;
        assert_eq!(state["success"], true);
    });
}

/// An unknown type answers `Unknown command` without the supplied id.
#[test]
fn unknown_commands_drop_supplied_ids() {
    on_task_set(|_| async {
        let (_h, _runtime, mut client) = served("unknown").await;
        for kind in ["unknown", "", "Get_State", "workspace"] {
            let line = json!({"id": "u", "type": kind});
            assert_eq!(
                client.ask(&line.to_string()).await,
                json!({"type": "response", "command": kind, "success": false,
                    "error": format!("Unknown command: {kind}")})
            );
        }
    });
}

/// Supplied shell operations produce the result and the stored shell message.
#[test]
fn custom_shell_output_reaches_saved_transcript() {
    on_task_set(|_| async {
        let (_h, runtime, _) = runtime_for("custom-shell", Vec::new()).await;
        let session = runtime.session();
        let options = BashRunOptions {
            operations: Some(Rc::new(ScriptedShell::output(
                "hello from custom ops",
                Some(0.0),
            ))),
            ..BashRunOptions::default()
        };
        let result = session.execute_bash("custom", None, options).await.unwrap();
        assert!(
            result.output.contains("hello from custom ops"),
            "{}",
            result.output
        );
        let messages = session.messages();
        let last = serde_json::to_value(messages.read().unwrap().last().unwrap()).unwrap();
        assert_eq!(last["role"], "bashExecution");
    });
}

/// Compaction forwards its instructions to the summary request and answers the stored
/// result unchanged.
#[test]
fn compaction_keeps_instructions_and_optional_details() {
    on_task_set(|_| async {
        let (h, runtime, mut client) = served("compact").await;
        let session = runtime.session();
        user_entry(&runtime, &"long context ".repeat(8000));
        user_entry(&runtime, "recent");
        let prompts = script_responses(&h, vec![answer("summary text", None, None)]);
        let line = r#"{"id":"c","type":"compact","customInstructions":"keep the names"}"#;
        let reply = client.ask(line).await;
        assert_eq!(
            (&reply["id"], &reply["success"]),
            (&json!("c"), &json!(true)),
            "{reply}"
        );
        let manager = session.session_manager().borrow();
        let entries = manager.get_entries();
        let stored = entries
            .iter()
            .rev()
            .find(|entry| entry["type"] == "compaction")
            .unwrap();
        let mut expected = json!({
            "summary": stored["summary"],
            "firstKeptEntryId": stored["firstKeptEntryId"],
            "tokensBefore": stored["tokensBefore"],
        });
        if let Some(details) = stored.get("details") {
            expected["details"] = details.clone();
        }
        if let Some(usage) = stored.get("usage") {
            expected["usage"] = usage.clone();
        }
        assert_eq!(reply["data"], wire(&expected));
        let asked = prompts.lock().unwrap().join("\n");
        assert!(asked.contains("keep the names"), "{asked}");
    });
}
