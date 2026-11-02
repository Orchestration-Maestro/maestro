//! The RPC client's commands on the wire, the answers they return and the records that
//! reach its event callbacks.
#![cfg(unix)]
use crate::rpc_peer::{Peer, SERVE, events, next_of};
use crate::support::rpc_connection::poll_once;
use crate::support::session_harness::on_task_set;
use maestro_app::{Presence, RpcClient, RpcClientError};
use serde::Serialize;
use serde_json::{Value, json};
use std::{cell::RefCell, pin::pin, rc::Rc};

/// A returned value as the JSON text the session would write for it.
fn value(result: &impl Serialize) -> Value {
    let text = maestro_json::compact_json(&serde_json::to_value(result).unwrap()).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// The string argument at `index`, empty when absent.
fn text(args: &[Value], index: usize) -> &str {
    args.get(index).and_then(Value::as_str).unwrap_or_default()
}

/// The optional argument at `index`.
fn optional<T: serde::de::DeserializeOwned>(args: &[Value], index: usize) -> Option<T> {
    args.get(index)
        .map(|arg| serde_json::from_value(arg.clone()).unwrap())
}

/// The shared result type of the method callers.
type Called = Result<Option<Value>, RpcClientError>;

/// Calls the client method named `method` with `args`; returns its result as JSON, or
/// `None` for a method that returns nothing and for missing text.
async fn call(client: &RpcClient, method: &str, args: &[Value]) -> Called {
    if NO_DATA.contains(&method) {
        return call_no_data(client, method, args).await.map(|()| None);
    }
    match session_data(client, method, args).await {
        Some(result) => result,
        None => branch_data(client, method, args).await,
    }
}

/// Calls a method that returns nothing.
async fn call_no_data(
    client: &RpcClient,
    method: &str,
    args: &[Value],
) -> Result<(), RpcClientError> {
    let parsed = |index: usize| args[index].clone();
    match method {
        "prompt" => client.prompt(text(args, 0), optional(args, 1)).await,
        "steer" => client.steer(text(args, 0), optional(args, 1)).await,
        "follow_up" => client.follow_up(text(args, 0), optional(args, 1)).await,
        "abort" => client.abort().await,
        "set_thinking_level" => {
            client
                .set_thinking_level(serde_json::from_value(parsed(0)).unwrap())
                .await
        }
        "set_steering_mode" => {
            client
                .set_steering_mode(serde_json::from_value(parsed(0)).unwrap())
                .await
        }
        "set_follow_up_mode" => {
            client
                .set_follow_up_mode(serde_json::from_value(parsed(0)).unwrap())
                .await
        }
        "set_auto_compaction" => client.set_auto_compaction(parsed(0) == true).await,
        "set_auto_retry" => client.set_auto_retry(parsed(0) == true).await,
        "abort_retry" => client.abort_retry().await,
        "abort_bash" => client.abort_bash().await,
        "set_session_name" => client.set_session_name(text(args, 0)).await,
        other => panic!("no client method {other}"),
    }
}

/// Calls a data method about the session's model and context; `None` for another method.
async fn session_data(client: &RpcClient, method: &str, args: &[Value]) -> Option<Called> {
    let optional_text = optional::<String>(args, 0);
    let optional_text = optional_text.as_deref();
    Some(match method {
        "new_session" => client
            .new_session(optional_text)
            .await
            .map(|r| Some(value(&r))),
        "get_state" => client.get_state().await.map(|r| Some(value(&r))),
        "set_model" => client
            .set_model(text(args, 0), text(args, 1))
            .await
            .map(|r| Some(value(&r))),
        "cycle_model" => client.cycle_model().await.map(|r| Some(value(&r))),
        "get_available_models" => client.get_available_models().await.map(|r| Some(value(&r))),
        "cycle_thinking_level" => client.cycle_thinking_level().await.map(|r| Some(value(&r))),
        "compact" => client.compact(optional_text).await.map(|r| Some(value(&r))),
        "bash" => client.bash(text(args, 0)).await.map(|r| Some(value(&r))),
        "get_session_stats" => client.get_session_stats().await.map(|r| Some(value(&r))),
        _ => return None,
    })
}

/// Calls a data method about the session's branches and records.
async fn branch_data(client: &RpcClient, method: &str, args: &[Value]) -> Called {
    match method {
        "export_html" => {
            let path = optional::<String>(args, 0);
            client
                .export_html(path.as_deref())
                .await
                .map(|r| Some(value(&r)))
        }
        "switch_session" => client
            .switch_session(text(args, 0))
            .await
            .map(|r| Some(value(&r))),
        "fork" => client.fork(text(args, 0)).await.map(|r| Some(value(&r))),
        "clone" => client.clone().await.map(|r| Some(value(&r))),
        "get_fork_messages" => client.get_fork_messages().await.map(|r| Some(value(&r))),
        "get_last_assistant_text" => {
            client
                .get_last_assistant_text()
                .await
                .map(|text| match text {
                    Presence::Missing => None,
                    Presence::Null => Some(Value::Null),
                    Presence::Present(text) => Some(json!(text)),
                })
        }
        "get_messages" => client.get_messages().await.map(|r| Some(value(&r))),
        "get_commands" => client.get_commands().await.map(|r| Some(value(&r))),
        other => panic!("no client method {other}"),
    }
}

/// The methods that return nothing.
const NO_DATA: [&str; 12] = [
    "prompt",
    "steer",
    "follow_up",
    "abort",
    "set_thinking_level",
    "set_steering_mode",
    "set_follow_up_mode",
    "set_auto_compaction",
    "set_auto_retry",
    "abort_retry",
    "abort_bash",
    "set_session_name",
];

/// Arguments that satisfy every method.
fn default_args(method: &str) -> Vec<Value> {
    match method {
        "set_thinking_level" => vec![json!("low")],
        "set_steering_mode" | "set_follow_up_mode" => vec![json!("all")],
        "set_auto_compaction" | "set_auto_retry" => vec![json!(true)],
        "prompt" | "steer" | "follow_up" => vec![json!("text")],
        _ => vec![json!("text"), json!("text")],
    }
}

/// A started client of `peer` running `script` with `PEER_ANSWERS` set to `answers`.
async fn started(peer: &Peer, script: &str, answers: &Value) -> RpcClient {
    let mut options = peer.options(script);
    options
        .env
        .insert("PEER_ANSWERS".into(), answers.to_string());
    let client = RpcClient::new(options);
    client.start().await.unwrap();
    client
}

#[test]
fn client_methods_match_the_command_corpus() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/rpc_client/commands.json")).unwrap();
    let peer = Peer::new();
    on_task_set(|_| async {
        let client = started(&peer, SERVE, &fixture["answers"]).await;
        let mut events = events(&client);
        let cases = fixture["cases"].as_array().unwrap();
        for (index, case) in cases.iter().enumerate() {
            let method = case["method"].as_str().unwrap();
            let result = call(&client, method, case["args"].as_array().unwrap())
                .await
                .unwrap();
            let wire = next_of(&mut events, "wire_capture").await;
            assert_eq!(wire["raw"], case["wire"], "case {index}");
            let expected = &case["result"];
            assert_eq!(
                result.is_some(),
                expected["present"] == true,
                "case {index}"
            );
            if let Some(result) = result {
                assert_eq!(result, expected["value"], "case {index}");
            }
        }
        assert_eq!(cases.len(), 59);
        client.stop().await.unwrap();
    });
}

#[test]
fn no_data_commands_reject_correlated_failure() {
    let peer = Peer::new();
    let refusing = "for line in commands():\n    command = json.loads(line)\n    respond(command, error='Denied ' + command['type'] + ' \\u00e9')";
    on_task_set(|_| async {
        let client = started(&peer, refusing, &json!({})).await;
        for method in NO_DATA
            .into_iter()
            .chain(["get_state", "cycle_model", "clone"])
        {
            let error = call(&client, method, &default_args(method))
                .await
                .unwrap_err();
            assert!(matches!(&error, RpcClientError::Response(_)), "{method}");
            assert_eq!(error.to_string(), format!("Denied {method} \u{e9}"));
        }
        client.stop().await.unwrap();
        let client = started(&peer, SERVE, &json!({})).await;
        for method in NO_DATA {
            assert_eq!(
                call(&client, method, &default_args(method)).await.unwrap(),
                None
            );
        }
        client.stop().await.unwrap();
    });
}

/// What the reference client returns for a successful `answer` to `command`.
fn returned(command: &str, answer: &Value) -> Option<Value> {
    let data = answer.get("data")?;
    let member = match command {
        "get_available_models" => "models",
        "get_fork_messages" | "get_messages" => "messages",
        "get_commands" => "commands",
        "get_last_assistant_text" => "text",
        _ => return Some(data.clone()),
    };
    data.get(member).cloned()
}

#[test]
fn returned_records_keep_their_complete_fields() {
    let corpus: Value =
        serde_json::from_str(include_str!("fixtures/rpc/command_responses.json")).unwrap();
    let cases: Vec<&Value> = corpus
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["input"]["type"] != "unknown")
        .collect();
    let answers: Vec<Value> = cases
        .iter()
        .map(|case| serde_json::from_str(case["raw"].as_str().unwrap()).unwrap())
        .collect();
    let replay = "for line, reply in zip(commands(), json.loads(os.environ['PEER_REPLIES'])):\n    reply['id'] = json.loads(line)['id']\n    emit(reply)\nsys.stdin.read()";
    let peer = Peer::new();
    on_task_set(|_| async {
        let mut options = peer.options(replay);
        options
            .env
            .insert("PEER_REPLIES".into(), json!(answers).to_string());
        let client = RpcClient::new(options);
        client.start().await.unwrap();
        for (case, answer) in cases.iter().zip(&answers) {
            let method = case["input"]["type"].as_str().unwrap();
            let result = call(&client, method, &default_args(method)).await;
            if answer["success"] == true {
                assert_eq!(
                    result.unwrap(),
                    returned(method, answer),
                    "{}",
                    case["name"]
                );
            } else {
                assert_eq!(
                    result.unwrap_err().to_string(),
                    answer["error"].as_str().unwrap()
                );
            }
        }
        assert_eq!(cases.len(), 46);
        client.stop().await.unwrap();
    });
}

#[test]
fn cycles_keep_null_and_selected_values() {
    let peer = Peer::new();
    let model = json!({"provider": "fixture", "id": "m", "contextWindow": 4096, "reasoning": true, "name": "Retained", "api": "openai-completions", "baseUrl": "https://fixture.invalid", "input": ["text", "image"], "maxTokens": 512, "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0}});
    let cycle = json!({"model": model, "thinkingLevel": "high", "isScoped": true});
    on_task_set(|_| async {
        for (method, data) in [
            ("cycle_model", Value::Null),
            ("cycle_model", cycle.clone()),
            ("cycle_thinking_level", Value::Null),
            ("cycle_thinking_level", json!({"level": "low"})),
        ] {
            let client = started(&peer, SERVE, &json!({ method: data })).await;
            assert_eq!(call(&client, method, &[]).await.unwrap(), Some(data));
            client.stop().await.unwrap();
        }
        let client = started(&peer, SERVE, &json!({"cycle_model": cycle})).await;
        let selected = client.cycle_model().await.unwrap().unwrap();
        assert!(selected.is_scoped);
        assert_eq!(selected.model.read().unwrap().id, "m");
        client.stop().await.unwrap();
        let client = started(&peer, SERVE, &json!({"cycle_thinking_level": null})).await;
        assert!(client.cycle_thinking_level().await.unwrap().is_none());
        client.stop().await.unwrap();
    });
}

#[test]
fn available_models_keep_complete_descriptors() {
    let peer = Peer::new();
    let first = json!({"provider": "fixture", "id": "a", "contextWindow": 4096, "reasoning": true, "name": "First", "api": "openai-completions", "baseUrl": "https://fixture.invalid", "input": ["text", "image"], "maxTokens": 512, "cost": {"input": 1.5, "output": 2, "cacheRead": 0, "cacheWrite": 0}});
    let mut second = first.clone();
    second["id"] = json!("b");
    second["reasoning"] = json!(false);
    on_task_set(|_| async {
        let answers = json!({"get_available_models": {"models": [first, second, first]}});
        let client = started(&peer, SERVE, &answers).await;
        let models: Vec<maestro_app::ModelInfo> = client
            .get_available_models()
            .await
            .unwrap()
            .iter()
            .map(|model| model.read().unwrap().clone())
            .collect();
        let ids: Vec<_> = models.iter().map(|model| model.id.as_str()).collect();
        assert_eq!(ids, ["a", "b", "a"]);
        let model = &models[0];
        assert_eq!(
            (
                model.provider.as_str(),
                model.context_window,
                model.reasoning
            ),
            ("fixture", 4096.0, true)
        );
        assert_eq!(
            (
                model.name.as_str(),
                model.base_url.as_str(),
                model.max_tokens
            ),
            ("First", "https://fixture.invalid", 512.0)
        );
        assert!(!models[1].reasoning);
        assert_eq!(value(model), first);
        client.stop().await.unwrap();
    });
}

#[test]
fn last_assistant_text_keeps_missing_null_and_text() {
    let peer = Peer::new();
    on_task_set(|_| async {
        for (data, expected) in [
            (json!({}), Presence::Missing),
            (json!({"text": null}), Presence::Null),
            (json!({"text": ""}), Presence::Present(String::new())),
            (
                json!({"text": "text"}),
                Presence::Present("text".to_owned()),
            ),
        ] {
            let client = started(&peer, SERVE, &json!({"get_last_assistant_text": data})).await;
            assert_eq!(client.get_last_assistant_text().await.unwrap(), expected);
            client.stop().await.unwrap();
        }
    });
}

#[test]
fn clone_round_trip_uses_the_active_branch_command() {
    let peer = Peer::new();
    on_task_set(|_| async {
        let client = started(&peer, SERVE, &json!({"clone": {"cancelled": false}})).await;
        let seen = Rc::new(RefCell::new(Vec::new()));
        let record = Rc::clone(&seen);
        let _remove = client.on_event(Rc::new(move |event: &Value| {
            record.borrow_mut().push(event["raw"].clone());
            Ok(())
        }));
        let cloned = client.clone().await.unwrap();
        assert!(!cloned.cancelled);
        let captures: Vec<_> = seen
            .borrow()
            .iter()
            .filter(|raw| !raw.is_null())
            .cloned()
            .collect();
        assert_eq!(captures, [json!("{\"type\":\"clone\",\"id\":\"req_1\"}\n")]);
        client.stop().await.unwrap();
    });
}

#[test]
fn responses_correlate_out_of_order() {
    let peer = Peer::new();
    let reversed = r"
lines = commands()
first, second = json.loads(next(lines)), json.loads(next(lines))
respond(second, {'path': 'second.html'})
respond(first, {'cancelled': True})
respond(first, {'cancelled': False})
emit({'type': 'done'})
sys.stdin.read()
";
    on_task_set(|_| async {
        let client = started(&peer, reversed, &json!({})).await;
        let mut events = events(&client);
        let (cloned, exported) = tokio::join!(client.clone(), client.export_html(None));
        assert!(cloned.unwrap().cancelled);
        assert_eq!(exported.unwrap().path, "second.html");
        let late = events.recv().await.unwrap();
        assert_eq!(
            late,
            json!({"type": "response", "id": "req_1", "command": "clone", "success": true, "data": {"cancelled": false}})
        );
        assert_eq!(events.recv().await.unwrap()["type"], "done");
        client.stop().await.unwrap();
    });
}

#[test]
fn pending_calls_ignore_null_and_malformed_lines_before_a_matching_reply() {
    let peer = Peer::new();
    let delayed = r"
lines = commands()
state = json.loads(next(lines))
sys.stdout.write('null\n{not json\n'); sys.stdout.flush()
emit({'type': 'barrier'})
abort = json.loads(next(lines))
respond(state, {'thinkingLevel': 'off', 'isStreaming': False, 'isCompacting': False, 'steeringMode': 'all', 'followUpMode': 'all', 'sessionId': 's', 'autoCompactionEnabled': False, 'messageCount': 0, 'pendingMessageCount': 0})
respond(abort)
sys.stdin.read()
";
    on_task_set(|_| async {
        let client = started(&peer, delayed, &json!({})).await;
        let mut events = events(&client);
        let mut state = pin!(client.get_state());
        assert!(poll_once(state.as_mut()).is_pending());
        assert_eq!(events.recv().await.unwrap(), json!({"type": "barrier"}));
        assert!(poll_once(state.as_mut()).is_pending());
        let (state, abort) = tokio::join!(state, client.abort());
        assert_eq!(state.unwrap().session_id, "s");
        abort.unwrap();
        assert!(events.try_recv().is_err());
        client.stop().await.unwrap();
    });
}

#[test]
fn unmatched_and_nonresponse_values_reach_listeners() {
    let peer = Peer::new();
    let records = [
        json!({}),
        json!([]),
        json!(42),
        json!(false),
        json!("text"),
        json!({"type": "response", "id": "req_99", "command": "abort", "success": true}),
        json!({"type": "response", "command": "abort", "success": true}),
        json!({"type": "message_update", "nested": {"deep": [1, {"x": null}]}}),
        json!({"type": "done"}),
    ];
    let script = "for record in json.loads(os.environ['PEER_RECORDS']):\n    sys.stdout.write('null\\n')\n    emit(record)\nsys.stdin.read()";
    on_task_set(|_| async {
        let mut options = peer.options(script);
        options
            .env
            .insert("PEER_RECORDS".into(), json!(records).to_string());
        let client = RpcClient::new(options);
        let mut events = events(&client);
        client.start().await.unwrap();
        for record in &records {
            assert_eq!(&events.recv().await.unwrap(), record);
        }
        client.stop().await.unwrap();
    });
}

#[test]
fn fragmented_utf8_and_jsonl_tails_keep_records() {
    let peer = Peer::new();
    let fragments = r#"
for chunk in [b'{"type":"a","text":"\xc3', b'\xa9\xf0\x9f', b'\x99\x82"}\r',
              b'\n{"type":"b","text":"\xe2\x80\xa8\xe2\x80\xa9"}\n{"type":"c"}\n{"type":',
              b'"tail"}']:
    os.write(1, chunk); time.sleep(0.01)
os.close(1)
sys.stdin.read()
"#;
    on_task_set(|_| async {
        let client = RpcClient::new(peer.options(fragments));
        let mut events = events(&client);
        client.start().await.unwrap();
        for expected in [
            json!({"type": "a", "text": "\u{e9}\u{1f642}"}),
            json!({"type": "b", "text": "\u{2028}\u{2029}"}),
            json!({"type": "c"}),
            json!({"type": "tail"}),
        ] {
            assert_eq!(events.recv().await.unwrap(), expected);
        }
        client.stop().await.unwrap();
    });
}
