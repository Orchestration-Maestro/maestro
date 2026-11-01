//! Real file notification delivery through the shipped file-trigger component and executable.
use crate::support::{Root, provider::Provider, rpc::Rpc};
use serde_json::json;

#[path = "../../../maestro-extensions-wasm/src/tests/file_trigger_path.rs"]
mod file_trigger_path;

#[test]
fn file_trigger_component_starts_a_turn_from_a_real_file_change() {
    let mut trigger = file_trigger_path::Trigger::new_in(std::path::Path::new("/tmp"));
    let root = Root::new();
    let provider = Provider::new();
    root.models(&provider.base);
    let component = std::env::var("MAESTRO_FILE_TRIGGER_COMPONENT")
        .expect("nextest author-component setup builds file_trigger");
    let mut rpc = Rpc::command(&mut root.command(&[
        "--mode",
        "rpc",
        "--provider",
        "startup",
        "--model",
        "controlled",
        "--extension",
        &component,
    ]));
    rpc.request(json!({"type":"get_state"}));
    assert!(
        rpc.events.iter().any(|event| {
            event["type"] == "extension_ui_request"
                && event["method"] == "notify"
                && event["message"] == "Watching /tmp/agent-trigger.txt"
        }),
        "session startup must acknowledge its live watch: {:?}",
        rpc.events
    );
    trigger.write(" \nRun the tests Ω\n ");
    loop {
        let event = rpc.read();
        let finished = event["type"] == "agent_end";
        rpc.events.push(event);
        if finished {
            break;
        }
    }
    assert_trigger(&mut rpc, &provider, &trigger);
    rpc.finish();
}

/// Checks the persisted custom message, one real provider request and the cleared file.
fn assert_trigger(rpc: &mut Rpc, provider: &Provider, trigger: &file_trigger_path::Trigger) {
    let messages = rpc.request(json!({"type":"get_messages"}));
    let messages = messages["messages"].as_array().unwrap();
    let custom: Vec<_> = messages
        .iter()
        .filter(|message| message["role"] == "custom")
        .collect();
    assert_eq!(custom.len(), 1, "one displayed external message");
    assert_eq!(custom[0]["customType"], "file-trigger");
    assert_eq!(custom[0]["content"], "External trigger: Run the tests Ω");
    assert_eq!(custom[0]["display"], true);
    let requests = provider.requests.lock().unwrap();
    assert_eq!(
        requests.len(),
        1,
        "triggerTurn must start one controlled response"
    );
    assert!(
        requests[0]
            .to_string()
            .contains("External trigger: Run the tests Ω")
    );
    drop(requests);
    assert_eq!(std::fs::read_to_string(trigger.path()).unwrap(), "");
}
