//! Exercises the shipped companion through the real executable's RPC connection.

use super::support::{Root, provider::Provider, rpc::Rpc};
use serde_json::{Value, json};

/// Reads and retains records until the requested dialog arrives.
fn dialog(rpc: &mut Rpc, method: &str) -> Value {
    loop {
        let record = rpc.read();
        rpc.events.push(record.clone());
        if record["type"] == "extension_ui_request" {
            if record["method"] == method {
                return record;
            }
            assert!(
                !["select", "confirm", "input", "editor"]
                    .contains(&record["method"].as_str().unwrap_or_default()),
                "{record}"
            );
        }
    }
}

/// Waits for one admitted command while retaining its notices and events.
fn response(rpc: &mut Rpc, id: &str) -> Value {
    if let Some(record) = rpc
        .events
        .iter()
        .find(|record| record["type"] == "response" && record["id"] == id)
    {
        assert_eq!(record["success"], true, "{record}");
        return record["data"].clone();
    }
    loop {
        let record = rpc.read();
        rpc.events.push(record.clone());
        if record["type"] == "response" && record["id"] == id {
            assert_eq!(record["success"], true, "{record}");
            return record["data"].clone();
        }
    }
}

/// Replies with the server's actual dialog identifier, never a guessed sequence number.
fn answer(rpc: &mut Rpc, request: &Value, mut fields: Value) {
    fields["type"] = "extension_ui_response".into();
    fields["id"] = request["id"].clone();
    rpc.send(&fields);
}

/// Sends a literal extension command without allowing a provider call to substitute for it.
fn command(rpc: &mut Rpc, id: &str, text: &str) {
    rpc.send(&json!({"type":"prompt", "id":id, "message":text}));
}

/// Asserts an independently specified notice was delivered over the protocol.
fn notice(rpc: &Rpc, method: &str, field: &str, value: &Value) {
    assert!(
        rpc.events
            .iter()
            .any(|record| record["type"] == "extension_ui_request"
                && record["method"] == method
                && &record[field] == value),
        "missing {method}/{field}: {}",
        json!(rpc.events)
    );
}

/// Opens a real RPC child with an isolated configuration and the shipped built component.
fn start(root: &Root) -> Rpc {
    let mut command = root.command(&[
        "--mode",
        "rpc",
        "--provider",
        "startup",
        "--model",
        "controlled",
        "--no-session",
        "--no-extensions",
        "--extension",
    ]);
    command
        .arg(std::env::var("MAESTRO_RPC_DEMO_COMPONENT").expect("nextest author-component setup"));
    Rpc::command(&mut command)
}

/// Exercises every non-model UI method, then one blocked and one harmless allowed tool call.
#[test]
fn rpc_demo_companion_exercises_dialogs_notices_and_lifecycle_over_rpc() {
    let root = Root::new();
    let provider = Provider::new();
    root.models(&provider.base);
    let mut rpc = start(&root);
    rpc.request(json!({"type":"get_state"}));
    notice(&rpc, "setTitle", "title", &json!("Maestro RPC Demo"));
    notice(&rpc, "setStatus", "statusText", &json!("Turns: 0"));
    notice(
        &rpc,
        "setWidget",
        "widgetLines",
        &json!(["--- RPC Extension UI Demo ---", "Loaded and ready."]),
    );
    text_dialogs(&mut rpc);
    switch_dialogs(&mut rpc);
    assert!(provider.requests.lock().unwrap().is_empty());
    tool_dialog(&mut rpc, "block", "Block");
    notice(&rpc, "notify", "message", &json!("Command blocked by user"));
    tool_dialog(&mut rpc, "allow", "Allow");
    notice(&rpc, "notify", "message", &json!("Command allowed"));
    notice(&rpc, "setStatus", "statusText", &json!("Turn 1 running..."));
    notice(&rpc, "setStatus", "statusText", &json!("Turn 1 done"));
    assert!(!provider.requests.lock().unwrap().is_empty());
    rpc.finish();
}

/// Keeps input/editor answers and editor-prefill notices distinct on the same connection.
fn text_dialogs(rpc: &mut Rpc) {
    command(rpc, "input", "/rpc-input");
    let request = dialog(rpc, "input");
    assert_eq!(request["title"], "Enter a value");
    assert_eq!(request["placeholder"], "type something...");
    answer(rpc, &request, json!({"value":"a value"}));
    response(rpc, "input");
    notice(rpc, "notify", "message", &json!("You entered: a value"));
    command(rpc, "editor", "/rpc-editor");
    let request = dialog(rpc, "editor");
    assert_eq!(request["prefill"], "Line 1\nLine 2\nLine 3");
    answer(rpc, &request, json!({"value":"one\ntwo"}));
    response(rpc, "editor");
    notice(
        rpc,
        "notify",
        "message",
        &json!("Editor submitted (2 lines)"),
    );
    command(rpc, "cancel-input", "/rpc-input");
    let request = dialog(rpc, "input");
    answer(rpc, &request, json!({"cancelled":true}));
    response(rpc, "cancel-input");
    notice(rpc, "notify", "message", &json!("Input cancelled"));
    command(rpc, "cancel-editor", "/rpc-editor");
    let request = dialog(rpc, "editor");
    answer(rpc, &request, json!({"cancelled":true}));
    response(rpc, "cancel-editor");
    notice(rpc, "notify", "message", &json!("Editor cancelled"));
    command(rpc, "prefill", "/rpc-prefill");
    response(rpc, "prefill");
    notice(
        rpc,
        "set_editor_text",
        "text",
        &json!("This text was set by the rpc-demo extension."),
    );
    notice(rpc, "notify", "message", &json!("Editor prefilled"));
}

/// Declines and accepts session replacement through correlated confirmation replies.
fn switch_dialogs(rpc: &mut Rpc) {
    let original = rpc.request(json!({"type":"get_state"}))["sessionId"].clone();
    rpc.send(&json!({"type":"new_session", "id":"decline"}));
    let request = dialog(rpc, "confirm");
    assert_eq!(request["title"], "Clear session?");
    assert_eq!(request["message"], "All messages will be lost.");
    answer(rpc, &request, json!({"confirmed":false}));
    assert_eq!(response(rpc, "decline")["cancelled"], true);
    assert_eq!(
        rpc.request(json!({"type":"get_state"}))["sessionId"],
        original
    );
    notice(rpc, "notify", "message", &json!("Clear cancelled"));
    rpc.send(&json!({"type":"new_session", "id":"accept"}));
    let request = dialog(rpc, "confirm");
    answer(rpc, &request, json!({"confirmed":true}));
    assert_eq!(response(rpc, "accept")["cancelled"], false);
    assert_ne!(
        rpc.request(json!({"type":"get_state"}))["sessionId"],
        original
    );
    notice(
        rpc,
        "setTitle",
        "title",
        &json!("Maestro RPC Demo (new session)"),
    );
}

/// The only model traffic goes to the controlled loopback provider; the command is just echo.
fn tool_dialog(rpc: &mut Rpc, id: &str, choice: &str) {
    let from = rpc.events.len();
    command(rpc, id, "rpc-demo-danger");
    let request = dialog(rpc, "select");
    assert_eq!(request["title"], "Dangerous command: echo sudo");
    assert_eq!(request["options"], json!(["Allow", "Block"]));
    answer(rpc, &request, json!({"value":choice}));
    response(rpc, id);
    while !rpc.events[from..]
        .iter()
        .any(|record| record["type"] == "agent_end")
    {
        let record = rpc.read();
        rpc.events.push(record);
    }
    rpc.request(json!({"type":"get_state"}));
}
