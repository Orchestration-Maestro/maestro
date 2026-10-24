//! The protocol records against recorded response lines and the shared workspace corpus.
#![cfg(not(target_arch = "wasm32"))]
use maestro_app::rpc_types::{
    RpcCommandBody, RpcData, RpcLastAssistantText, RpcSessionCommand, parse_rpc_command,
};
use maestro_app::{Presence, RpcCommand, RpcResponse, RpcSessionState};
use maestro_json::jsonl::serialize_json_line;
use serde_json::{Value, json};

/// One recorded command, its input and the exact line answering it.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// The case name.
    name: String,
    /// The command record.
    input: Value,
    /// The response line, with its LF.
    raw: String,
}

/// The recorded command cases.
fn cases() -> Vec<Case> {
    serde_json::from_str(include_str!("fixtures/rpc/command_responses.json")).unwrap()
}

/// Every recorded response line decodes into its typed record and is written back with the
/// same bytes.
#[test]
fn session_response_bytes_match_corpus() {
    let cases = cases();
    let commands: std::collections::BTreeSet<&str> = cases
        .iter()
        .filter_map(|case| case.input["type"].as_str())
        .collect();
    assert_eq!(
        commands.len(),
        30,
        "the 29 session commands and an unknown one"
    );
    for case in &cases {
        let response: RpcResponse = serde_json::from_str(&case.raw).unwrap();
        let written = serialize_json_line(&serde_json::to_value(&response).unwrap()).unwrap();
        assert_eq!(written, case.raw, "{}", case.name);
    }
    let decoded: RpcResponse = serde_json::from_str(
        r#"{"type":"response","command":"cycle_model","success":true,"data":null}"#,
    )
    .unwrap();
    assert!(matches!(
        decoded,
        RpcResponse::Success {
            data: RpcData::Null,
            ..
        }
    ));
    for (data, text) in [
        ("{}", Presence::Missing),
        (r#"{"text":null}"#, Presence::Null),
        (r#"{"text":"a"}"#, Presence::Present("a".to_owned())),
    ] {
        let line = format!(
            r#"{{"type":"response","command":"get_last_assistant_text","success":true,"data":{data}}}"#
        );
        let decoded: RpcResponse = serde_json::from_str(&line).unwrap();
        let RpcResponse::Success {
            data: RpcData::LastAssistantText(decoded),
            ..
        } = decoded
        else {
            panic!("{data}");
        };
        assert_eq!(decoded, RpcLastAssistantText { text }, "{data}");
    }
    let decoded: RpcResponse =
        serde_json::from_str(r#"{"type":"response","command":"abort","success":true}"#).unwrap();
    assert!(matches!(
        decoded,
        RpcResponse::Success {
            data: RpcData::Empty,
            ..
        }
    ));
}

/// Session commands and every workspace command decode through the one `type` tag and are
/// written back unchanged; a method and params dialect is not a command.
#[test]
fn typed_command_union_keeps_workspace_requests() {
    for case in cases().iter().filter(|case| case.name != "unknown") {
        let command: RpcCommand = serde_json::from_value(case.input.clone()).unwrap();
        assert!(
            matches!(command.command, RpcCommandBody::Session(_)),
            "{}",
            case.name
        );
        assert_eq!(
            serde_json::to_value(&command).unwrap(),
            case.input,
            "{}",
            case.name
        );
    }
    let corpus: Value = serde_json::from_str(include_str!(
        "../../maestro-protocol/tests/fixtures/workspace_wire.json"
    ))
    .unwrap();
    let requests = corpus["requests"].as_array().unwrap();
    assert_eq!(requests.len(), 92);
    for request in requests {
        let command = parse_rpc_command(request).unwrap();
        assert!(
            matches!(command.command, RpcCommandBody::Workspace(_)),
            "{request}"
        );
        let written = serde_json::to_value(&command).unwrap();
        let workspace: maestro_app::rpc_types::workspace::Request =
            serde_json::from_value(request.clone()).unwrap();
        assert_eq!(
            written,
            serde_json::to_value(workspace).unwrap(),
            "{request}"
        );
    }
    let dialect = json!({"id": "m", "method": "get_state", "params": {}});
    assert!(serde_json::from_value::<RpcCommand>(dialect).is_err());
    let state = json!({"type": "get_state"});
    assert!(matches!(
        parse_rpc_command(&state).unwrap().command,
        RpcCommandBody::Session(RpcSessionCommand::GetState)
    ));
}

/// A state without model, file or name writes none of those members.
#[test]
fn optional_state_fields_remain_omitted() {
    let state = RpcSessionState {
        model: None,
        thinking_level: maestro_models::ModelThinkingLevel::Medium,
        is_streaming: false,
        is_compacting: false,
        steering_mode: maestro_agent::QueueMode::All,
        follow_up_mode: maestro_agent::QueueMode::OneAtATime,
        session_file: None,
        session_id: "session-a".into(),
        session_name: None,
        auto_compaction_enabled: true,
        message_count: 0,
        pending_message_count: 0,
    };
    let case = cases()
        .into_iter()
        .find(|case| case.name == "state_omitted")
        .unwrap();
    let response = RpcResponse::Success {
        id: None,
        command: "get_state".into(),
        data: RpcData::State(state),
    };
    let written = serialize_json_line(&serde_json::to_value(&response).unwrap()).unwrap();
    assert_eq!(written, case.raw);
}
