//! Shared workspace wire contract.
#![cfg(test)]

use maestro_protocol::schema::Request;

#[test]
/// Exercise the shared public wire or binary codec.
fn workspace_commands_use_top_level_type_and_parameters() {
    let raw = r#"{"id":"w1","type":"workspace.create","cwd":"/workspace","focus":true}"#;
    let request: Request = serde_json::from_str(raw).unwrap();
    let value = serde_json::to_value(request).unwrap();
    assert_eq!(value["type"], "workspace.create");
    assert_eq!(value["cwd"], "/workspace");
    assert_eq!(value["focus"], true);
    assert!(value.get("method").is_none());
    assert!(value.get("params").is_none());
}

#[test]
/// Exercise the shared public wire or binary codec.
fn action_manifest_link_ignores_obsolete_source_and_defaults_enabled() {
    let request: Request = serde_json::from_value(serde_json::json!({
        "type": "action.manifest.link", "path": "/actions", "source": {"kind": "github"}
    }))
    .unwrap();
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({
            "type": "action.manifest.link", "path": "/actions", "enabled": true
        })
    );
}

#[test]
/// Exercise the shared public wire or binary codec.
fn workspace_responses_preserve_correlations_and_open_error_codes() {
    use maestro_protocol::rpc_types::RpcResponse;
    for raw in [
        r#"{"id":"w1","type":"response","command":"workspace.create","success":true,"data":{"type":"ok"}}"#,
        r#"{"type":"response","command":"workspace.create","success":false,"error":"unavailable","code":"future_failure_kind"}"#,
    ] {
        let response: RpcResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(serde_json::to_string(&response).unwrap(), raw);
    }
}

#[test]
/// Exercise the shared public wire or binary codec.
fn workspace_events_have_one_type_tag_without_an_outer_wrapper() {
    use maestro_protocol::schema::EventData;
    for raw in [
        r#"{"type":"pane_output_matched","pane_id":"pane","matched_line":"ready","read":{"pane_id":"pane","workspace_id":"work","tab_id":"tab","source":"recent","format":"text","text":"ready","revision":7,"truncated":false}}"#,
        r#"{"type":"pane_scroll_changed","pane_id":"pane","workspace_id":"work","scroll":{"offset_from_bottom":2,"max_offset_from_bottom":9,"viewport_rows":24}}"#,
    ] {
        let event: EventData = serde_json::from_str(raw).unwrap();
        assert_eq!(serde_json::to_string(&event).unwrap(), raw);
    }
}

#[test]
/// Exercise the shared public wire or binary codec.
fn local_keymaps_preserve_prefix_multiple_keys_and_disabled_actions_in_binary() {
    use maestro_protocol::frames::{
        ClientKeybindings, ClientLaunchMode, ClientMessage, RenderEncoding,
    };
    let bindings = std::collections::BTreeMap::from([
        ("prefix".into(), vec!["ctrl+b".into()]),
        (
            "workspace.next".into(),
            vec!["alt+n".into(), "ctrl+right".into()],
        ),
        ("workspace.close".into(), vec![]),
    ]);
    for keybindings in [
        ClientKeybindings::Server,
        ClientKeybindings::Local { bindings },
    ] {
        let hello = ClientMessage::Hello {
            version: maestro_protocol::PROTOCOL_VERSION,
            cols: 80,
            rows: 24,
            cell_width_px: 0,
            cell_height_px: 0,
            requested_encoding: RenderEncoding::SemanticFrame,
            keybindings,
            launch_mode: ClientLaunchMode::App,
        };
        let mut bytes = vec![];
        maestro_protocol::frames::write_message(&mut bytes, &hello).unwrap();
        let decoded: ClientMessage = maestro_protocol::frames::read_message(
            &mut bytes.as_slice(),
            maestro_protocol::frames::MAX_FRAME_SIZE,
        )
        .unwrap();
        assert_eq!(decoded, hello);
    }
    assert_eq!(maestro_protocol::PROTOCOL_VERSION, 21);
}

/// Readable preservation corpus for the workspace command and event family.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkspaceCorpus {
    /// One input for each workspace command.
    requests: Vec<serde_json::Value>,
    /// Each existing typed success payload.
    results: Vec<serde_json::Value>,
    /// Each ordinary workspace event payload.
    events: Vec<serde_json::Value>,
    /// Every subscription selector.
    subscriptions: Vec<serde_json::Value>,
}

/// Load the corpus without silently discarding fixture envelope members.
fn corpus() -> WorkspaceCorpus {
    serde_json::from_str(include_str!("fixtures/workspace_wire.json")).unwrap()
}

/// Observe every supplied field; serialization may add the documented defaults.
fn assert_fields_preserved(input: &serde_json::Value, output: &serde_json::Value) {
    match input {
        serde_json::Value::Object(fields) => {
            for (key, value) in fields {
                assert_fields_preserved(value, output.get(key).unwrap());
            }
        }
        serde_json::Value::Array(values) => {
            let output = output.as_array().unwrap();
            assert_eq!(values.len(), output.len());
            for (value, actual) in values.iter().zip(output) {
                assert_fields_preserved(value, actual);
            }
        }
        _ => assert_eq!(input, output),
    }
}

#[test]
/// Every workspace command preserves its parameters and correlates both outcomes.
fn all_workspace_commands_and_correlated_responses_round_trip() {
    use maestro_protocol::rpc_types::RpcResponse;
    use maestro_protocol::schema::{ErrorBody, ResponseResult};
    let requests = corpus().requests;
    assert_eq!(requests.len(), 92);
    let mut commands = std::collections::BTreeSet::new();
    for value in requests {
        let request: Request = serde_json::from_value(value.clone()).unwrap();
        let encoded = serde_json::to_value(&request).unwrap();
        assert_fields_preserved(&value, &encoded);
        assert_eq!(request, serde_json::from_value(encoded).unwrap());
        let command = value["type"].as_str().unwrap();
        assert!(commands.insert(command.to_owned()));
        for result in [
            Ok(ResponseResult::Ok {}),
            Err(ErrorBody {
                code: "new_machine_code".into(),
                message: "operation refused".into(),
            }),
        ] {
            let response = RpcResponse {
                id: request.id.clone(),
                command: command.into(),
                result,
            };
            let encoded = serde_json::to_value(&response).unwrap();
            assert_eq!(encoded["command"], command);
            assert_eq!(encoded["id"], value["id"]);
            assert_eq!(encoded["success"], response.result.is_ok());
            assert_eq!(
                serde_json::from_value::<RpcResponse>(encoded).unwrap(),
                response
            );
        }
    }
}

#[test]
/// Every existing workspace result, event and selector retains its supplied fields.
fn workspace_payloads_and_subscription_selectors_preserve_fields() {
    use maestro_protocol::schema::{EventData, ResponseResult, Subscription};
    let corpus = corpus();
    assert_eq!(corpus.results.len(), 58);
    assert_eq!(corpus.events.len(), 26);
    assert_eq!(corpus.subscriptions.len(), 27);
    for value in corpus.results {
        let result: ResponseResult = serde_json::from_value(value.clone()).unwrap();
        let encoded = serde_json::to_value(&result).unwrap();
        assert_fields_preserved(&value, &encoded);
        assert_eq!(result, serde_json::from_value(encoded).unwrap());
    }
    for value in corpus.events {
        let event: EventData = serde_json::from_value(value.clone()).unwrap();
        let encoded = serde_json::to_value(&event).unwrap();
        assert_fields_preserved(&value, &encoded);
        assert_eq!(event, serde_json::from_value(encoded).unwrap());
    }
    for value in corpus.subscriptions {
        let selector: Subscription = serde_json::from_value(value.clone()).unwrap();
        let encoded = serde_json::to_value(&selector).unwrap();
        assert_fields_preserved(&value, &encoded);
        assert_eq!(selector, serde_json::from_value(encoded).unwrap());
    }
}

#[test]
/// The single request family rejects removed names and malformed known parameters.
fn removed_envelopes_and_names_are_not_accepted() {
    for name in [
        "plugin.link",
        "plugin.list",
        "plugin.unlink",
        "plugin.enable",
        "plugin.disable",
        "plugin.action.list",
        "plugin.action.invoke",
        "plugin.log.list",
        "plugin.pane.open",
        "plugin.pane.focus",
        "plugin.pane.close",
        "not_a_command",
    ] {
        assert!(serde_json::from_value::<Request>(serde_json::json!({"type": name})).is_err());
    }
    for value in [
        serde_json::json!({"id":"old", "method":"ping", "params":{}}),
        serde_json::json!({"type":"workspace.create", "focus":"yes"}),
        serde_json::json!({"type":"pane.send_text", "pane_id":"p"}),
        serde_json::json!({"id":7, "type":"ping"}),
    ] {
        assert!(serde_json::from_value::<Request>(value).is_err());
    }
    let request: Request =
        serde_json::from_value(serde_json::json!({"type":"ping","future":{}})).unwrap();
    assert!(request.id.is_none());
    assert_eq!(
        serde_json::to_string(&request).unwrap(),
        r#"{"type":"ping"}"#
    );
}

#[test]
/// Package sources preserve checkout metadata and omit absent installation time.
fn local_git_and_npm_sources_round_trip() {
    use maestro_protocol::schema::PackageSourceInfo;
    for raw in [
        serde_json::json!({"kind":"local","source":"/actions"}),
        serde_json::json!({"kind":"git","source":"git:example/actions","subdir":"tools","requested_ref":"stable","resolved_commit":"revision","managed_path":"/managed/actions","installed_unix_ms":19}),
        serde_json::json!({"kind":"npm","source":"npm:actions","requested_ref":"1.2.3","installed_unix_ms":20}),
    ] {
        let source: PackageSourceInfo = serde_json::from_value(raw.clone()).unwrap();
        assert_eq!(serde_json::to_value(source).unwrap(), raw);
    }
}

#[test]
/// Success and failure markers are required, with the typed payload for their branch.
fn workspace_responses_reject_conflicting_or_legacy_shapes() {
    use maestro_protocol::rpc_types::RpcResponse;
    for value in [
        serde_json::json!({"id":"old", "result":{"type":"ok"}}),
        serde_json::json!({"type":"response","command":"ping","success":true,"error":"failed","code":"no"}),
        serde_json::json!({"type":"response","command":"ping","success":false,"data":{"type":"ok"}}),
        serde_json::json!({"type":"response","command":"ping","success":false,"error":{"code":"no","message":"failed"}}),
        serde_json::json!({"type":"response","command":"ping","success":"true","data":{"type":"ok"}}),
    ] {
        assert!(serde_json::from_value::<RpcResponse>(value).is_err());
    }
}

#[test]
/// Every public workspace command, including graphics streaming, appears in the schema.
fn published_schema_covers_the_delivered_command_family() {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../../docs/daemon-protocol.schema.json")).unwrap();
    let commands = schema["schemas"]["request"]["oneOf"].as_array().unwrap();
    let names: std::collections::BTreeSet<_> = commands
        .iter()
        .map(|variant| variant["properties"]["type"]["const"].as_str().unwrap())
        .collect();
    for request in corpus().requests {
        assert!(names.contains(request["type"].as_str().unwrap()));
    }
    assert_eq!(names.len(), 92);
    assert_eq!(schema["protocol"], 21);
}
