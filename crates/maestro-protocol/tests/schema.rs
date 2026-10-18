//! JSON requests, responses and generated schema.
//! Adapted for Maestro: names, module layout and Rust documentation changed.
#![cfg(test)]
use std::collections::HashMap;

use maestro_protocol::schema::*;

/// Protocol schema entry.
fn protocol_schema_entry<T: schemars::JsonSchema>(name: &str) -> serde_json::Value {
    let mut schema = serde_json::to_value(schemars::schema_for!(T)).unwrap();
    rewrite_schema_refs(&mut schema, name);
    schema
}

/// Rewrite schema refs.
fn rewrite_schema_refs(value: &mut serde_json::Value, schema_name: &str) {
    match value {
        serde_json::Value::Object(object) => {
            if let Some(serde_json::Value::String(reference)) = object.get_mut("$ref")
                && let Some(path) = reference.strip_prefix("#/")
            {
                *reference = format!("#/schemas/{schema_name}/{path}");
            }
            for child in object.values_mut() {
                rewrite_schema_refs(child, schema_name);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                rewrite_schema_refs(item, schema_name);
            }
        }
        _ => {}
    }
}

/// Protocol schema document.
fn protocol_schema_document() -> serde_json::Value {
    serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "Maestro API",
        "schema_version": 1,
        "protocol": maestro_protocol::PROTOCOL_VERSION,
        "schemas": {
            "request": protocol_schema_entry::<Request>("request"),
            "response": protocol_schema_entry::<RpcResponse>("response"),
            "event": protocol_schema_entry::<EventData>("event"),
        },
    })
}

#[test]
/// Request uses dot method names.
fn request_method_serializes_with_a_dot_separator() {
    let request = Request {
        id: Some("req_1".into()),
        command: RpcCommand::WorkspaceCreate(WorkspaceCreateParams {
            cwd: Some("/tmp".into()),
            focus: true,
            label: Some("api".into()),
            env: HashMap::default(),
        }),
    };

    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["type"], "workspace.create");
}

#[test]
/// Pane input accepts both target spellings and emits the Maestro spelling.
fn pane_input_normalizes_target_alias() {
    for target in ["maestro", "herdr"] {
        let request: Request = serde_json::from_value(serde_json::json!({
            "id": "input",
            "type": "pane.input.set",
            "pane_id": "p_1", "right_click": target
        }))
        .unwrap();
        let RpcCommand::PaneInputSet(params) = &request.command else {
            panic!("wrong method parsed");
        };
        assert_eq!(params.right_click, PaneRightClickTarget::Maestro);
        assert_eq!(
            serde_json::to_value(&request).unwrap()["right_click"],
            "maestro"
        );
    }
    assert_eq!(
        serde_json::to_value(PaneRightClickTarget::Pane).unwrap(),
        "pane"
    );
}

#[test]
/// Agent start and prompt requests round trip.
fn agent_launch_and_prompt_options_survive_json() {
    let start = Request {
        id: Some("start".into()),
        command: RpcCommand::AgentStart(AgentStartParams {
            name: "reviewer".into(),
            kind: "maestro".into(),
            pane_id: "w1:p2".into(),
            args: vec!["--no-session".into()],
            timeout_ms: Some(30_000),
        }),
    };
    let start_json = serde_json::to_value(&start).unwrap();
    assert_eq!(start_json["type"], "agent.start");
    assert_eq!(start_json["pane_id"], "w1:p2");
    assert_eq!(
        serde_json::from_value::<Request>(start_json).unwrap(),
        start
    );

    let prompt = Request {
        id: Some("prompt".into()),
        command: RpcCommand::AgentPrompt(AgentPromptParams {
            target: "reviewer".into(),
            text: "review this".into(),
            wait: None,
        }),
    };
    let prompt_json = serde_json::to_value(&prompt).unwrap();
    assert_eq!(prompt_json["type"], "agent.prompt");
    assert_eq!(
        serde_json::from_value::<Request>(prompt_json).unwrap(),
        prompt
    );

    let prompt_and_wait = Request {
        id: Some("prompt-and-wait".into()),
        command: RpcCommand::AgentPrompt(AgentPromptParams {
            target: "reviewer".into(),
            text: "review this".into(),
            wait: Some(AgentPromptWaitOptions {
                until: vec![AgentStatus::Idle, AgentStatus::Done],
                timeout_ms: Some(120_000),
            }),
        }),
    };
    let prompt_and_wait_json = serde_json::to_value(&prompt_and_wait).unwrap();
    assert_eq!(
        prompt_and_wait_json["wait"]["until"],
        serde_json::json!(["idle", "done"])
    );
    assert_eq!(prompt_and_wait_json["wait"]["timeout_ms"], 120_000);
    assert_eq!(
        serde_json::from_value::<Request>(prompt_and_wait_json).unwrap(),
        prompt_and_wait
    );
}

#[test]
/// Bundled schema references do not use standalone definition prefixes.
fn schema_bundle_avoids_standalone_definition_prefixes() {
    /// Assert no standalone refs.
    fn assert_no_standalone_refs(value: &serde_json::Value) {
        match value {
            serde_json::Value::Object(object) => {
                if let Some(serde_json::Value::String(reference)) = object.get("$ref") {
                    assert!(
                        !reference.starts_with("#/$defs/"),
                        "schema bundle contains standalone ref {reference}"
                    );
                }
                for child in object.values() {
                    assert_no_standalone_refs(child);
                }
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    assert_no_standalone_refs(item);
                }
            }
            _ => {}
        }
    }

    assert_no_standalone_refs(&protocol_schema_document());
}

#[test]
/// Generated protocol schema artifact is current.
fn schema_bundle_matches_the_published_document() {
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("../../../docs/daemon-protocol.schema.json")).unwrap();
    assert_eq!(expected, protocol_schema_document());
}

#[test]
/// Request round trips for server stop.
fn server_stop_preserves_its_request_envelope() {
    let request = Request {
        id: Some("req_stop".into()),
        command: RpcCommand::ServerStop(EmptyParams::default()),
    };

    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["type"], "server.stop");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, request);
}

#[test]
/// Request round trips for server reload config.
fn config_reload_preserves_its_request_envelope() {
    let request = Request {
        id: Some("req_reload".into()),
        command: RpcCommand::ServerReloadConfig(EmptyParams::default()),
    };

    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["type"], "server.reload_config");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, request);
}

#[test]
/// Request round trips for server reload agent manifests.
fn manifest_reload_preserves_its_request_envelope() {
    let request = Request {
        id: Some("req_reload_agent_manifests".into()),
        command: RpcCommand::ServerReloadAgentManifests(EmptyParams::default()),
    };

    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["type"], "server.reload_agent_manifests");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, request);
}

#[test]
/// Request round trips for server agent manifests.
fn manifest_listing_preserves_its_request_envelope() {
    let request = Request {
        id: Some("req_agent_manifests".into()),
        command: RpcCommand::ServerAgentManifests(EmptyParams::default()),
    };

    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["type"], "server.agent_manifests");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, request);
}

#[test]
/// Request round trips for agent explain.
fn agent_explanation_preserves_its_request_envelope() {
    let request = Request {
        id: Some("req_agent_explain".into()),
        command: RpcCommand::AgentExplain(AgentTarget {
            target: "agent-1".into(),
        }),
    };

    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["type"], "agent.explain");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, request);
}

#[test]
/// Notification show request parses.
fn notification_json_decodes_its_presentation_options() {
    let json = r#"{
  "id": "req_1",
  "type": "notification.show",
  "title": "build failed",
  "body": "api workspace",
  "position": "top-left",
  "sound": "request"
}"#;
    let request: Request = serde_json::from_str(json).unwrap();
    let RpcCommand::NotificationShow(params) = request.command else {
        panic!("wrong method parsed");
    };
    assert_eq!(params.title, "build failed");
    assert_eq!(params.body.as_deref(), Some("api workspace"));
    assert_eq!(
        params.position,
        Some(maestro_protocol::shared::ToastMaestroPosition::TopLeft)
    );
    assert_eq!(params.sound, NotificationShowSound::Request);
}

#[test]
/// Notification show sound defaults to none.
fn omitted_notification_sound_selects_none() {
    let json = r#"{
  "id": "req_1",
  "type": "notification.show",
  "title": "build failed"
}"#;
    let request: Request = serde_json::from_str(json).unwrap();
    let RpcCommand::NotificationShow(params) = request.command else {
        panic!("wrong method parsed");
    };

    assert_eq!(params.sound, NotificationShowSound::None);
}

#[test]
/// Client window title requests round trip.
fn window_title_commands_preserve_their_json_records() {
    let set = Request {
        id: Some("req_title_set".into()),
        command: RpcCommand::ClientWindowTitleSet(ClientWindowTitleSetParams {
            title: "maestro api".into(),
        }),
    };
    let json = serde_json::to_value(&set).unwrap();
    assert_eq!(json["type"], "client.window_title.set");
    assert_eq!(json["title"], "maestro api");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, set);

    let clear = Request {
        id: Some("req_title_clear".into()),
        command: RpcCommand::ClientWindowTitleClear(EmptyParams::default()),
    };
    let json = serde_json::to_value(&clear).unwrap();
    assert_eq!(json["type"], "client.window_title.clear");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, clear);
}

#[test]
/// Agent view requests round trip.
fn agent_view_commands_preserve_their_json_records() {
    let set_json = serde_json::json!({
        "id": "view-set",
        "type": "agent.view.set",
        "source": "example.views",
            "label": "current + attention",
            "filter": {
                "op": "any",
                "filters": [
                    {
                        "op": "eq",
                        "field": "workspace_id",
                        "value": {"context": "current_workspace_id"}
                    },
                    {
                        "op": "in",
                        "field": "status",
                        "values": ["blocked", "done"]
                    }
                ]
            },
            "sort": [
                {"field": "attention", "order": "desc"},
                {"field": "state_change_seq", "order": "desc"}
            ]
    });
    let request: Request = serde_json::from_value(set_json.clone()).unwrap();
    assert!(matches!(request.command, RpcCommand::AgentViewSet(_)));
    assert_eq!(serde_json::to_value(request).unwrap(), set_json);

    let clear_json = serde_json::json!({
        "id": "view-clear",
        "type": "agent.view.clear",
        "source": "example.views"
    });
    let request: Request = serde_json::from_value(clear_json.clone()).unwrap();
    assert!(matches!(request.command, RpcCommand::AgentViewClear(_)));
    assert_eq!(serde_json::to_value(request).unwrap(), clear_json);
}

#[test]
/// Unknown method is rejected.
fn unregistered_method_names_fail_request_decoding() {
    let json = r#"{
  "id": "req_1",
  "type": "nope"
}"#;
    let err = serde_json::from_str::<Request>(json)
        .unwrap_err()
        .to_string();
    assert!(err.contains("unknown variant"));
}

#[test]
/// Missing required params are rejected.
fn absent_mandatory_parameters_fail_request_decoding() {
    let json = r#"{
  "id": "req_1",
  "type": "pane.send_text",
  "pane_id": "p_1"
}"#;
    let err = serde_json::from_str::<Request>(json)
        .unwrap_err()
        .to_string();
    assert!(err.contains("text"));
}

#[test]
/// Pane send input defaults to empty text and keys.
fn omitted_pane_input_fields_produce_empty_values() {
    let json = r#"{
  "id": "req_1",
  "type": "pane.send_input",
  "pane_id": "p_1"
}"#;

    let request: Request = serde_json::from_str(json).unwrap();
    let RpcCommand::PaneSendInput(params) = request.command else {
        panic!("wrong method parsed");
    };
    assert_eq!(params.pane_id, "p_1");
    assert!(params.text.is_empty());
    assert!(params.keys.is_empty());
}

#[test]
/// Pane wait for output defaults strip ansi to true.
fn omitted_output_wait_option_enables_ansi_stripping() {
    let json = r#"{
  "id": "req_1",
  "type": "pane.wait_for_output",
  "pane_id": "p_1",
  "source": "recent",
  "match": {
    "type": "substring",
    "value": "ready"
  }
}"#;

    let request: Request = serde_json::from_str(json).unwrap();
    let RpcCommand::PaneWaitForOutput(params) = request.command else {
        panic!("wrong method parsed");
    };
    assert!(params.strip_ansi);
}

#[test]
/// Pane read defaults to text format.
fn omitted_pane_read_format_selects_text() {
    let json = r#"{
  "id": "req_1",
  "type": "pane.read",
  "pane_id": "p_1",
  "source": "visible"
}"#;

    let request: Request = serde_json::from_str(json).unwrap();
    let serialized = serde_json::to_value(&request).unwrap();
    assert!(serialized.get("intent").is_none());
    let RpcCommand::PaneRead(params) = request.command else {
        panic!("wrong method parsed");
    };
    assert_eq!(params.format, ReadFormat::Text);
    assert_eq!(params.intent, ReadIntent::Interactive);
}

#[test]
/// Pane current request round trips.
fn current_pane_lookup_preserves_its_request_envelope() {
    let request = Request {
        id: Some("req_current".into()),
        command: RpcCommand::PaneCurrent(PaneCurrentParams {
            caller_pane_id: Some("w1-1".into()),
        }),
    };

    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["type"], "pane.current");
    assert_eq!(json["caller_pane_id"], "w1-1");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, request);
}

#[test]
/// Pane process info request round trips.
fn pane_process_lookup_preserves_its_request_envelope() {
    let request = Request {
        id: Some("req_process_info".into()),
        command: RpcCommand::PaneProcessInfo(PaneProcessInfoParams {
            pane_id: Some("w1-1".into()),
        }),
    };

    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["type"], "pane.process_info");
    assert_eq!(json["pane_id"], "w1-1");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, request);
}

#[test]
/// Event envelope round trips.
fn event_json_preserves_payload_and_envelope() {
    let events = [
        EventData::PaneOutputChanged {
            pane_id: "p_1".into(),
            workspace_id: "w_1".into(),
            revision: 42,
        },
        EventData::WorkspaceMoved {
            workspace_id: "w_1".into(),
            insert_index: 2,
            workspaces: vec![],
        },
        EventData::WorkspaceReordered {
            workspace_ids: vec!["w_1".into(), "w_2".into()],
            before_workspace_id: Some("w_3".into()),
            workspaces: vec![],
        },
        EventData::TabMoved {
            tab_id: "w_1:1".into(),
            workspace_id: "w_1".into(),
            insert_index: 1,
            tabs: vec![],
        },
        EventData::LayoutUpdated {
            layout: event_layout(),
        },
    ];

    for event in events {
        let json = serde_json::to_string(&event).unwrap();
        let restored: EventData = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, event);
    }
}

#[test]
/// Subscribe request parses parameterized subscriptions.
fn subscription_json_decodes_event_parameters() {
    let json = r#"{
  "id": "sub_1",
  "type": "events.subscribe",
  "subscriptions": [
    {
      "type": "pane.output_matched",
      "pane_id": "p_1_1",
      "source": "recent",
      "lines": 200,
      "match": {
        "type": "substring",
        "value": "auth: received"
      }
    },
    {
      "type": "pane.agent_status_changed",
      "pane_id": "p_1_1",
      "agent_status": "done"
    },
    {
      "type": "pane.scroll_changed",
      "pane_id": "p_1_1"
    }
  ]
}"#;

    let request: Request = serde_json::from_str(json).unwrap();
    let RpcCommand::EventsSubscribe(params) = request.command else {
        panic!("wrong method parsed");
    };
    assert_eq!(params.subscriptions.len(), 3);
    assert!(matches!(
        &params.subscriptions[0],
        Subscription::PaneOutputMatched {
            pane_id,
            source: ReadSource::Recent,
            lines: Some(200),
            r#match: OutputMatch::Substring { value },
            strip_ansi: true,
        } if pane_id == "p_1_1" && value == "auth: received"
    ));
    assert!(matches!(
        &params.subscriptions[1],
        Subscription::PaneAgentStatusChanged {
            pane_id,
            agent_status: Some(AgentStatus::Done),
        } if pane_id == "p_1_1"
    ));
    assert!(matches!(
        &params.subscriptions[2],
        Subscription::PaneScrollChanged { pane_id } if pane_id == "p_1_1"
    ));
}

#[test]
/// Subscription event envelope round trips.
fn subscription_delivery_preserves_its_json_envelope() {
    let event = EventData::PaneOutputMatched(PaneOutputMatchedEvent {
        pane_id: "p_1_1".into(),
        matched_line: "auth: received".into(),
        read: PaneReadResult {
            pane_id: "p_1_1".into(),
            workspace_id: "w_1".into(),
            tab_id: "t_1_1".into(),
            source: ReadSource::Recent,
            format: ReadFormat::Text,
            text: "auth: received\n".into(),
            revision: 0,
            truncated: false,
        },
    });

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("\"type\":\"pane_output_matched\""));
    let restored: EventData = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, event);
}

#[test]
/// Scroll changed subscription event round trips.
fn scroll_delivery_preserves_its_subscription_payload() {
    let event = EventData::PaneScrollChanged(PaneScrollChangedEvent {
        pane_id: "p_1_1".into(),
        workspace_id: "w_1".into(),
        scroll: PaneScrollInfo {
            offset_from_bottom: 12,
            max_offset_from_bottom: 240,
            viewport_rows: 30,
        },
    });

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("\"type\":\"pane_scroll_changed\""));
    let restored: EventData = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, event);
}

#[test]
/// Success response round trips.
fn successful_reply_survives_json_encoding() {
    let response = RpcResponse {
        id: Some("req_1".into()),
        command: "ping".into(),
        result: Ok(ResponseResult::Pong {
            version: "0.1.2".into(),
            protocol: 6,
            capabilities: Some(ServerCapabilities {
                live_handoff: true,
                detached_server_daemon: true,
            }),
        }),
    };

    let json = serde_json::to_string(&response).unwrap();
    let restored: RpcResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, response);
}

#[test]
/// Session snapshot request and response round trip.
fn session_snapshot_exchange_preserves_json_records() {
    let request = Request {
        id: Some("req_snapshot".into()),
        command: RpcCommand::SessionSnapshot(EmptyParams::default()),
    };
    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains("\"type\":\"session.snapshot\""));
    let restored: Request = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, request);

    let response = RpcResponse {
        id: Some("req_snapshot".into()),
        command: "session.snapshot".into(),
        result: Ok(ResponseResult::SessionSnapshot {
            snapshot: Box::new(SessionSnapshot {
                version: "0.1.2".into(),
                protocol: 16,
                focused_workspace_id: None,
                focused_tab_id: None,
                focused_pane_id: None,
                workspaces: Vec::new(),
                tabs: Vec::new(),
                panes: Vec::new(),
                layouts: Vec::new(),
                agents: Vec::new(),
            }),
        }),
    };
    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("\"type\":\"session_snapshot\""));
    let restored: RpcResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, response);
}

#[test]
/// Worktree request and response round trip.
fn worktree_exchange_preserves_json_records() {
    let request = Request {
        id: Some("req_worktree".into()),
        command: RpcCommand::WorktreeCreate(WorktreeCreateParams {
            workspace_id: Some("1".into()),
            branch: Some("worktree/api".into()),
            base: Some("HEAD".into()),
            focus: true,
            ..WorktreeCreateParams::default()
        }),
    };
    let json = serde_json::to_string(&request).unwrap();
    let restored: Request = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, request);

    let response = RpcResponse {
        id: Some("req_worktree".into()),
        command: "worktree.create".into(),
        result: Ok(ResponseResult::WorktreeCreated {
            workspace: WorkspaceInfo {
                workspace_id: "w_1".into(),
                number: 2,
                label: "maestro".into(),
                focused: true,
                pane_count: 1,
                tab_count: 1,
                active_tab_id: "w_1:1".into(),
                agent_status: AgentStatus::Unknown,
                tokens: HashMap::new(),
                worktree: Some(WorkspaceWorktreeInfo {
                    repo_key: "/repo/maestro/.git".into(),
                    repo_name: "maestro".into(),
                    repo_root: "/repo/maestro".into(),
                    checkout_path: "/worktrees/maestro/worktree-api".into(),
                    is_linked_worktree: true,
                }),
            },
            tab: TabInfo {
                tab_id: "w_1:1".into(),
                workspace_id: "w_1".into(),
                number: 1,
                label: "maestro".into(),
                focused: true,
                pane_count: 1,
                agent_status: AgentStatus::Unknown,
            },
            root_pane: worktree_root_pane(),
            worktree: linked_worktree("w_1"),
        }),
    };
    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("\"type\":\"worktree_created\""));
    assert!(json.contains("\"worktree\""));
    let restored: RpcResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, response);
}

#[test]
/// Worktree lifecycle events round trip.
fn worktree_notifications_preserve_lifecycle_payloads() {
    let subscription = Request {
        id: Some("sub_worktrees".into()),
        command: RpcCommand::EventsSubscribe(EventsSubscribeParams {
            subscriptions: vec![
                Subscription::WorktreeCreated {},
                Subscription::WorktreeOpened {},
                Subscription::WorktreeRemoved {},
            ],
        }),
    };
    let json = serde_json::to_string(&subscription).unwrap();
    assert!(json.contains("\"type\":\"worktree.created\""));
    assert!(json.contains("\"type\":\"worktree.opened\""));
    assert!(json.contains("\"type\":\"worktree.removed\""));
    let restored: Request = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, subscription);

    let workspace = lifecycle_workspace();
    let worktree = linked_worktree("w_2");

    for event in [
        EventData::WorktreeCreated {
            workspace: workspace.clone(),
            worktree: worktree.clone(),
        },
        EventData::WorktreeOpened {
            workspace: workspace.clone(),
            worktree: worktree.clone(),
            already_open: false,
        },
        EventData::WorktreeRemoved {
            workspace_id: "w_2".into(),
            workspace: Some(workspace.clone()),
            worktree: WorktreeInfo {
                open_workspace_id: None,
                ..worktree
            },
            forced: false,
        },
        EventData::WorkspaceClosed {
            workspace_id: "w_2".into(),
            workspace: Some(workspace),
        },
    ] {
        let json = serde_json::to_string(&event).unwrap();
        let restored: EventData = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, event);
    }
}

#[test]
/// Action link list unlink round trip.
fn action_registration_commands_preserve_json_records() {
    let link = Request {
        id: Some("action_link".into()),
        command: RpcCommand::ActionManifestLink(ActionManifestLinkParams {
            path: "/manifests/worktree-bootstrap".into(),
            enabled: true,
        }),
    };
    let json = serde_json::to_string(&link).unwrap();
    assert!(json.contains("\"type\":\"action.manifest.link\""));
    let restored: Request = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, link);

    let list = Request {
        id: Some("action_list".into()),
        command: RpcCommand::ActionManifestList(ActionManifestListParams {
            manifest_id: Some("example.worktree-bootstrap".into()),
        }),
    };
    let json = serde_json::to_string(&list).unwrap();
    assert!(json.contains("\"type\":\"action.manifest.list\""));
    let restored: Request = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, list);

    let unlink = Request {
        id: Some("action_unlink".into()),
        command: RpcCommand::ActionManifestUnlink(ActionManifestUnlinkParams {
            manifest_id: "example.worktree-bootstrap".into(),
        }),
    };
    let json = serde_json::to_string(&unlink).unwrap();
    assert!(json.contains("\"type\":\"action.manifest.unlink\""));
    let restored: Request = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, unlink);

    let manifest = installed_manifest();

    for response in [
        RpcResponse {
            id: Some("action_link".into()),
            command: "action.manifest.link".into(),
            result: Ok(ResponseResult::ActionManifestLinked {
                manifest: manifest.clone(),
            }),
        },
        RpcResponse {
            id: Some("action_list".into()),
            command: "action.manifest.list".into(),
            result: Ok(ResponseResult::ActionManifestList {
                manifests: vec![manifest.clone()],
            }),
        },
        RpcResponse {
            id: Some("action_unlink".into()),
            command: "action.manifest.unlink".into(),
            result: Ok(ResponseResult::ActionManifestUnlinked {
                manifest_id: manifest.manifest_id,
                removed: true,
            }),
        },
    ] {
        let json = serde_json::to_string(&response).unwrap();
        let restored: RpcResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, response);
    }
}

#[test]
/// Action version input aliases normalize to the Maestro field on output.
fn action_version_normalizes_input_alias() {
    let manifest = installed_manifest();
    let mut json = serde_json::to_value(&manifest).unwrap();
    assert_eq!(json["min_maestro_version"], manifest.min_maestro_version);
    assert!(json.get("min_herdr_version").is_none());
    assert_eq!(
        serde_json::from_value::<ActionManifestInfo>(json.clone()).unwrap(),
        manifest
    );
    let version = json
        .as_object_mut()
        .unwrap()
        .shift_remove("min_maestro_version")
        .unwrap();
    json["min_herdr_version"] = version;
    let restored: ActionManifestInfo = serde_json::from_value(json).unwrap();
    assert_eq!(restored, manifest);
    assert!(
        serde_json::to_value(restored)
            .unwrap()
            .get("min_herdr_version")
            .is_none()
    );
}

#[test]
/// Layout export apply round trip.
fn layout_exchange_preserves_tree_records() {
    let root = LayoutNode::Split {
        direction: SplitDirection::Right,
        ratio: 0.6,
        first: Box::new(LayoutNode::Pane {
            pane: LayoutPane {
                label: Some("editor".into()),
                cwd: Some("/repo".into()),
                ..Default::default()
            },
        }),
        second: Box::new(LayoutNode::Pane {
            pane: LayoutPane {
                label: Some("tests".into()),
                command: Some(vec!["sh".into(), "-c".into(), "just test".into()]),
                env: HashMap::from([("MAESTRO_ROLE".into(), "tests".into())]),
                ..Default::default()
            },
        }),
    };

    let export = Request {
        id: Some("layout_export".into()),
        command: RpcCommand::LayoutExport(LayoutExportParams {
            tab_id: Some("w1:1".into()),
            pane_id: None,
        }),
    };
    let json = serde_json::to_string(&export).unwrap();
    assert!(json.contains("\"type\":\"layout.export\""));
    let restored: Request = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, export);

    let apply = Request {
        id: Some("layout_apply".into()),
        command: RpcCommand::LayoutApply(LayoutApplyParams {
            workspace_id: Some("w1".into()),
            tab_id: None,
            tab_label: Some("dev".into()),
            focus: true,
            root: root.clone(),
        }),
    };
    let json = serde_json::to_string(&apply).unwrap();
    assert!(json.contains("\"type\":\"layout.apply\""));
    let restored: Request = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, apply);

    assert_layout_responses(root);
}

#[test]
/// Authority mutation requests round trip.
fn authority_commands_preserve_json_parameters() {
    let workspace_move = Request {
        id: Some("move_ws".into()),
        command: RpcCommand::WorkspaceMove(WorkspaceMoveParams {
            workspace_id: "w1".into(),
            insert_index: 2,
        }),
    };
    let json = serde_json::to_value(&workspace_move).unwrap();
    assert_eq!(json["type"], "workspace.move");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, workspace_move);

    let workspace_move_block = Request {
        id: Some("move_ws_block".into()),
        command: RpcCommand::WorkspaceMoveBlock(WorkspaceMoveBlockParams {
            workspace_ids: vec!["w1".into(), "w2".into()],
            before_workspace_id: Some("w3".into()),
        }),
    };
    let json = serde_json::to_value(&workspace_move_block).unwrap();
    assert_eq!(json["type"], "workspace.move_block");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, workspace_move_block);

    let tab_move = Request {
        id: Some("move_tab".into()),
        command: RpcCommand::TabMove(TabMoveParams {
            tab_id: "w1:1".into(),
            insert_index: 1,
        }),
    };
    let json = serde_json::to_value(&tab_move).unwrap();
    assert_eq!(json["type"], "tab.move");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, tab_move);

    assert_pane_authority_requests();
}

#[test]
/// Create response round trips with root pane.
fn creation_reply_preserves_the_root_pane() {
    let response = RpcResponse {
        id: Some("req_2".into()),
        command: "tab.create".into(),
        result: Ok(ResponseResult::TabCreated {
            tab: TabInfo {
                tab_id: "w_1:2".into(),
                workspace_id: "w_1".into(),
                number: 2,
                label: "review".into(),
                focused: false,
                pane_count: 1,
                agent_status: AgentStatus::Unknown,
            },
            root_pane: PaneInfo {
                pane_id: "w_1-3".into(),
                terminal_id: "term_example".into(),
                workspace_id: "w_1".into(),
                tab_id: "w_1:2".into(),
                focused: false,
                cwd: Some("/tmp/review".into()),
                foreground_cwd: None,
                label: None,
                agent: None,
                title: None,
                terminal_title: None,
                terminal_title_stripped: None,
                display_agent: None,
                agent_status: AgentStatus::Unknown,
                state_labels: HashMap::new(),
                tokens: HashMap::new(),
                agent_session: None,
                scroll: None,
                revision: 0,
            },
        }),
    };

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("\"type\":\"tab_created\""));
    assert!(json.contains("\"root_pane\""));
    let restored: RpcResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, response);
}

#[test]
/// Error response round trips.
fn failure_reply_preserves_error_fields() {
    let response = RpcResponse {
        id: Some("req_1".into()),
        command: "pane.get".into(),
        result: Err(ErrorBody {
            code: "pane_not_found".into(),
            message: "pane p_1 not found".into(),
        }),
    };

    let json = serde_json::to_string(&response).unwrap();
    let restored: RpcResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, response);
}

#[test]
/// Event wait parses typed match.
fn event_wait_decodes_typed_matching_options() {
    let json = r#"{
  "id": "req_9",
  "type": "events.wait",
  "match_event": {
    "event": "pane_agent_status_changed",
    "pane_id": "p_1",
    "agent_status": "done"
  },
  "timeout_ms": 30000
}"#;

    let request: Request = serde_json::from_str(json).unwrap();
    let RpcCommand::EventsWait(params) = request.command else {
        panic!("wrong method parsed");
    };
    assert_eq!(
        params.match_event,
        EventMatch::PaneAgentStatusChanged {
            pane_id: "p_1".into(),
            agent_status: AgentStatus::Done,
        }
    );
}

#[test]
/// Action list and invoke round trips.
fn action_exchange_preserves_json_records() {
    let list = Request {
        id: Some("req_action_list".into()),
        command: RpcCommand::ActionList(ActionListParams {
            manifest_id: Some("example.issue-flow".into()),
        }),
    };
    let json = serde_json::to_value(&list).unwrap();
    assert_eq!(json["type"], "action.list");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, list);

    let invoke = Request {
        id: Some("req_action_invoke".into()),
        command: RpcCommand::ActionInvoke(ActionInvokeParams {
            manifest_id: Some("example.issue-flow".into()),
            action_id: "assign-issue".into(),
            context: None,
        }),
    };
    let json = serde_json::to_value(&invoke).unwrap();
    assert_eq!(json["type"], "action.invoke");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, invoke);

    let action_info = ActionInfo {
        manifest_id: "example.issue-flow".into(),
        action_id: "assign-issue".into(),
        title: "Assign Issue".into(),
        description: Some("Open the issue assignment UI".into()),
        contexts: vec![ActionContext::Workspace, ActionContext::Pane],
        command: vec!["assign".into(), "--issue".into()],
        platforms: Some(vec![ActionPlatform::Linux, ActionPlatform::Macos]),
    };
    assert_eq!(
        action_info.qualified_id(),
        "example.issue-flow.assign-issue"
    );
    let json = serde_json::to_string(&action_info).unwrap();
    let restored: ActionInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, action_info);
}

#[test]
/// Action pane open request round trips.
fn action_pane_request_preserves_popup_options() {
    let request = Request {
        id: Some("req_action_pane".into()),
        command: RpcCommand::ActionPaneOpen(ActionPaneOpenParams {
            manifest_id: "example.board".into(),
            entrypoint: "board".into(),
            placement: Some(ActionPanePlacement::Popup),
            width: Some(maestro_protocol::shared::PopupSize::Cells(90)),
            height: Some(maestro_protocol::shared::PopupSize::Percent(80)),
            workspace_id: None,
            target_pane_id: None,
            direction: None,
            cwd: Some("/tmp".into()),
            focus: true,
            env: [("MAESTRO_ROLE".to_string(), "board".to_string())].into(),
        }),
    };

    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["type"], "action.pane.open");
    assert_eq!(json["placement"], "popup");
    assert_eq!(json["width"], 90);
    assert_eq!(json["height"], "80%");
    assert_eq!(json["env"]["MAESTRO_ROLE"], "board");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, request);
}

#[test]
/// Popup close emits its method name and empty parameters.
fn popup_close_serializes_the_method_and_empty_parameters() {
    let request = Request {
        id: Some("popup-close".into()),
        command: RpcCommand::PopupClose(EmptyParams::default()),
    };

    let json = serde_json::to_value(request).unwrap();

    assert_eq!(json["type"], "popup.close");
    assert_eq!(json.as_object().unwrap().len(), 2);
}

/// Event layout.
fn event_layout() -> PaneLayoutSnapshot {
    PaneLayoutSnapshot {
        workspace_id: "w_1".into(),
        tab_id: "w_1:1".into(),
        zoomed: false,
        area: PaneLayoutRect {
            x: 0,
            y: 0,
            width: 100,
            height: 24,
        },
        focused_pane_id: "w_1-1".into(),
        panes: vec![PaneLayoutPane {
            pane_id: "w_1-1".into(),
            focused: true,
            rect: PaneLayoutRect {
                x: 0,
                y: 0,
                width: 100,
                height: 24,
            },
        }],
        splits: vec![],
    }
}

/// Root pane created with a worktree.
fn worktree_root_pane() -> PaneInfo {
    PaneInfo {
        pane_id: "w_1-1".into(),
        terminal_id: "term_1".into(),
        workspace_id: "w_1".into(),
        tab_id: "w_1:1".into(),
        focused: true,
        cwd: Some("/worktrees/maestro/worktree-api".into()),
        foreground_cwd: None,
        label: None,
        agent: None,
        title: None,
        terminal_title: None,
        terminal_title_stripped: None,
        display_agent: None,
        agent_status: AgentStatus::Unknown,
        state_labels: HashMap::new(),
        tokens: HashMap::new(),
        agent_session: None,
        scroll: None,
        revision: 0,
    }
}

/// Workspace used by lifecycle events.
fn lifecycle_workspace() -> WorkspaceInfo {
    WorkspaceInfo {
        workspace_id: "w_2".into(),
        number: 2,
        label: "maestro".into(),
        focused: true,
        pane_count: 1,
        tab_count: 1,
        active_tab_id: "w_2:1".into(),
        agent_status: AgentStatus::Unknown,
        tokens: HashMap::new(),
        worktree: Some(WorkspaceWorktreeInfo {
            repo_key: "/repo/maestro/.git".into(),
            repo_name: "maestro".into(),
            repo_root: "/repo/maestro".into(),
            checkout_path: "/worktrees/maestro/worktree-api".into(),
            is_linked_worktree: true,
        }),
    }
}

/// Linked workspace action bundle.
fn installed_manifest() -> ActionManifestInfo {
    ActionManifestInfo {
        manifest_id: "example.worktree-bootstrap".into(),
        name: "Worktree Bootstrap".into(),
        version: "0.1.0".into(),
        min_maestro_version: maestro_protocol::build_info::BASE_VERSION.into(),
        description: Some("Prepare new worktrees".into()),
        manifest_path: "/manifests/worktree-bootstrap/maestro-manifest.toml".into(),
        manifest_root: "/manifests/worktree-bootstrap".into(),
        enabled: true,
        platforms: None,
        build: vec![ActionManifestBuild {
            platforms: None,
            command: vec!["bun".into(), "install".into()],
        }],
        startup: vec![],
        actions: vec![ActionManifestAction {
            id: "bootstrap".into(),
            title: "Bootstrap worktree".into(),
            description: None,
            contexts: vec![ActionContext::Workspace],
            platforms: None,
            command: vec!["bun".into(), "run".into(), "bootstrap.ts".into()],
        }],
        events: vec![ActionManifestEventHook {
            on: "worktree.created".into(),
            platforms: None,
            command: vec!["bun".into(), "run".into(), "bootstrap.ts".into()],
        }],
        panes: vec![ActionManifestPane {
            id: "board".into(),
            title: "Board".into(),
            description: None,
            platforms: None,
            placement: ActionPanePlacement::Overlay,
            width: None,
            height: None,
            command: vec!["bun".into(), "run".into(), "board.ts".into()],
        }],
        link_handlers: vec![ActionManifestLinkHandler {
            id: "github-pr".into(),
            title: "Open GitHub PR".into(),
            pattern: "^https://github.com/[^/]+/[^/]+/(issues|pull)/[0-9]+$".into(),
            action: "bootstrap".into(),
            platforms: None,
        }],
        source: PackageSourceInfo::default(),
        warnings: vec![],
    }
}

/// Layout result envelopes.
fn assert_layout_responses(root: LayoutNode) {
    let response = RpcResponse {
        id: Some("layout_export".into()),
        command: "layout.export".into(),
        result: Ok(ResponseResult::LayoutExport {
            layout: LayoutDescription {
                workspace_id: "w1".into(),
                tab_id: "w1:1".into(),
                zoomed: false,
                focused_pane_id: "w1-1".into(),
                root,
            },
        }),
    };
    let json = serde_json::to_string(&response).unwrap();
    let restored: RpcResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, response);

    let response = RpcResponse {
        id: Some("layout_ratio".into()),
        command: "layout.set_split_ratio".into(),
        result: Ok(ResponseResult::LayoutSplitRatioSet {
            layout: LayoutDescription {
                workspace_id: "w1".into(),
                tab_id: "w1:1".into(),
                zoomed: false,
                focused_pane_id: "w1-1".into(),
                root: LayoutNode::Pane {
                    pane: LayoutPane {
                        pane_id: Some("w1-1".into()),
                        ..Default::default()
                    },
                },
            },
        }),
    };
    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("\"type\":\"layout_split_ratio_set\""));
    let restored: RpcResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, response);
}

/// Pane authority and subscription request shapes.
fn assert_pane_authority_requests() {
    let pane_focus = Request {
        id: Some("focus_pane".into()),
        command: RpcCommand::PaneFocus(PaneTarget {
            pane_id: "w1:1".into(),
        }),
    };
    let json = serde_json::to_value(&pane_focus).unwrap();
    assert_eq!(json["type"], "pane.focus");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, pane_focus);

    let split_ratio = Request {
        id: Some("set_ratio".into()),
        command: RpcCommand::LayoutSetSplitRatio(LayoutSetSplitRatioParams {
            tab_id: Some("w1:1".into()),
            pane_id: None,
            path: vec![false, true],
            ratio: 0.6,
        }),
    };
    let json = serde_json::to_value(&split_ratio).unwrap();
    assert_eq!(json["type"], "layout.set_split_ratio");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, split_ratio);

    let subscription = Request {
        id: Some("sub_moves".into()),
        command: RpcCommand::EventsSubscribe(EventsSubscribeParams {
            subscriptions: vec![
                Subscription::WorkspaceMoved {},
                Subscription::WorkspaceReordered {},
                Subscription::TabMoved {},
                Subscription::LayoutUpdated {},
            ],
        }),
    };
    let json = serde_json::to_string(&subscription).unwrap();
    assert!(json.contains("\"type\":\"workspace.moved\""));
    assert!(json.contains("\"type\":\"workspace.reordered\""));
    assert!(json.contains("\"type\":\"tab.moved\""));
    assert!(json.contains("\"type\":\"layout.updated\""));
    let restored: Request = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, subscription);
}

/// Linked checkout attached to the named workspace.
fn linked_worktree(workspace_id: &str) -> WorktreeInfo {
    WorktreeInfo {
        path: "/worktrees/maestro/worktree-api".into(),
        branch: Some("worktree/api".into()),
        is_bare: false,
        is_detached: false,
        is_prunable: false,
        is_linked_worktree: true.into(),
        open_workspace_id: Some(workspace_id.into()),
        label: "maestro".into(),
    }
}
