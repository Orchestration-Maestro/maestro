//! Each command over the protocol, answered by a real session runtime.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::rpc_connection::connect;
use crate::support::runtime_fixtures::runtime_for;
use crate::support::session_harness::on_task_set;
use maestro_app::RpcConnectionOutcome;
use serde_json::{Value, json};
use std::rc::Rc;

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
