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
        "protocol": maestro_protocol::frames::PROTOCOL_VERSION,
        "schemas": {
            "request": protocol_schema_entry::<Request>("request"),
            "success_response": protocol_schema_entry::<SuccessResponse>("success_response"),
            "error_response": protocol_schema_entry::<ErrorResponse>("error_response"),
            "event": protocol_schema_entry::<EventEnvelope>("event"),
            "subscription_event": protocol_schema_entry::<SubscriptionEventEnvelope>("subscription_event"),
        },
    })
}

#[test]
/// Request uses dot method names.
fn request_method_serializes_with_a_dot_separator() {
    let request = Request {
        id: "req_1".into(),
        method: Method::WorkspaceCreate(WorkspaceCreateParams {
            cwd: Some("/tmp".into()),
            focus: true,
            label: Some("api".into()),
            env: HashMap::default(),
        }),
    };

    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["method"], "workspace.create");
}

#[test]
/// Agent start and prompt requests round trip.
fn agent_launch_and_prompt_options_survive_json() {
    let start = Request {
        id: "start".into(),
        method: Method::AgentStart(AgentStartParams {
            name: "reviewer".into(),
            kind: "maestro".into(),
            pane_id: "w1:p2".into(),
            args: vec!["--no-session".into()],
            timeout_ms: Some(30_000),
        }),
    };
    let start_json = serde_json::to_value(&start).unwrap();
    assert_eq!(start_json["method"], "agent.start");
    assert_eq!(start_json["params"]["pane_id"], "w1:p2");
    assert_eq!(
        serde_json::from_value::<Request>(start_json).unwrap(),
        start
    );

    let prompt = Request {
        id: "prompt".into(),
        method: Method::AgentPrompt(AgentPromptParams {
            target: "reviewer".into(),
            text: "review this".into(),
            wait: None,
        }),
    };
    let prompt_json = serde_json::to_value(&prompt).unwrap();
    assert_eq!(prompt_json["method"], "agent.prompt");
    assert_eq!(
        serde_json::from_value::<Request>(prompt_json).unwrap(),
        prompt
    );

    let prompt_and_wait = Request {
        id: "prompt-and-wait".into(),
        method: Method::AgentPrompt(AgentPromptParams {
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
        prompt_and_wait_json["params"]["wait"]["until"],
        serde_json::json!(["idle", "done"])
    );
    assert_eq!(
        prompt_and_wait_json["params"]["wait"]["timeout_ms"],
        120_000
    );
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
        id: "req_stop".into(),
        method: Method::ServerStop(EmptyParams::default()),
    };

    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["method"], "server.stop");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, request);
}

#[test]
/// Request round trips for server reload config.
fn config_reload_preserves_its_request_envelope() {
    let request = Request {
        id: "req_reload".into(),
        method: Method::ServerReloadConfig(EmptyParams::default()),
    };

    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["method"], "server.reload_config");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, request);
}

#[test]
/// Request round trips for server reload agent manifests.
fn manifest_reload_preserves_its_request_envelope() {
    let request = Request {
        id: "req_reload_agent_manifests".into(),
        method: Method::ServerReloadAgentManifests(EmptyParams::default()),
    };

    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["method"], "server.reload_agent_manifests");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, request);
}

#[test]
/// Request round trips for server agent manifests.
fn manifest_listing_preserves_its_request_envelope() {
    let request = Request {
        id: "req_agent_manifests".into(),
        method: Method::ServerAgentManifests(EmptyParams::default()),
    };

    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["method"], "server.agent_manifests");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, request);
}

#[test]
/// Request round trips for agent explain.
fn agent_explanation_preserves_its_request_envelope() {
    let request = Request {
        id: "req_agent_explain".into(),
        method: Method::AgentExplain(AgentTarget {
            target: "agent-1".into(),
        }),
    };

    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["method"], "agent.explain");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, request);
}

#[test]
/// Notification show request parses.
fn notification_json_decodes_its_presentation_options() {
    let json = r#"{"id":"req_1","method":"notification.show","params":{"title":"build failed","body":"api workspace","position":"top-left","sound":"request"}}"#;
    let request: Request = serde_json::from_str(json).unwrap();
    let Method::NotificationShow(params) = request.method else {
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
    let json = r#"{"id":"req_1","method":"notification.show","params":{"title":"build failed"}}"#;
    let request: Request = serde_json::from_str(json).unwrap();
    let Method::NotificationShow(params) = request.method else {
        panic!("wrong method parsed");
    };

    assert_eq!(params.sound, NotificationShowSound::None);
}

#[test]
/// Client window title requests round trip.
fn window_title_commands_preserve_their_json_records() {
    let set = Request {
        id: "req_title_set".into(),
        method: Method::ClientWindowTitleSet(ClientWindowTitleSetParams {
            title: "maestro api".into(),
        }),
    };
    let json = serde_json::to_value(&set).unwrap();
    assert_eq!(json["method"], "client.window_title.set");
    assert_eq!(json["params"]["title"], "maestro api");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, set);

    let clear = Request {
        id: "req_title_clear".into(),
        method: Method::ClientWindowTitleClear(EmptyParams::default()),
    };
    let json = serde_json::to_value(&clear).unwrap();
    assert_eq!(json["method"], "client.window_title.clear");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, clear);
}

#[test]
/// Agent view requests round trip.
fn agent_view_commands_preserve_their_json_records() {
    let set_json = serde_json::json!({
        "id": "view-set",
        "method": "agent.view.set",
        "params": {
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
        }
    });
    let request: Request = serde_json::from_value(set_json.clone()).unwrap();
    assert!(matches!(request.method, Method::AgentViewSet(_)));
    assert_eq!(serde_json::to_value(request).unwrap(), set_json);

    let clear_json = serde_json::json!({
        "id": "view-clear",
        "method": "agent.view.clear",
        "params": {"source": "example.views"}
    });
    let request: Request = serde_json::from_value(clear_json.clone()).unwrap();
    assert!(matches!(request.method, Method::AgentViewClear(_)));
    assert_eq!(serde_json::to_value(request).unwrap(), clear_json);
}

#[test]
/// Unknown method is rejected.
fn unregistered_method_names_fail_request_decoding() {
    let json = r#"{"id":"req_1","method":"nope","params":{}}"#;
    let err = serde_json::from_str::<Request>(json)
        .unwrap_err()
        .to_string();
    assert!(err.contains("unknown variant"));
}

#[test]
/// Missing required params are rejected.
fn absent_mandatory_parameters_fail_request_decoding() {
    let json = r#"{"id":"req_1","method":"pane.send_text","params":{"pane_id":"p_1"}}"#;
    let err = serde_json::from_str::<Request>(json)
        .unwrap_err()
        .to_string();
    assert!(err.contains("text"));
}

#[test]
/// Pane send input defaults to empty text and keys.
fn omitted_pane_input_fields_produce_empty_values() {
    let json = r#"
    {
        "id": "req_1",
        "method": "pane.send_input",
        "params": {
            "pane_id": "p_1"
        }
    }
    "#;

    let request: Request = serde_json::from_str(json).unwrap();
    let Method::PaneSendInput(params) = request.method else {
        panic!("wrong method parsed");
    };
    assert_eq!(params.pane_id, "p_1");
    assert!(params.text.is_empty());
    assert!(params.keys.is_empty());
}

#[test]
/// Pane wait for output defaults strip ansi to true.
fn omitted_output_wait_option_enables_ansi_stripping() {
    let json = r#"
    {
        "id": "req_1",
        "method": "pane.wait_for_output",
        "params": {
            "pane_id": "p_1",
            "source": "recent",
            "match": { "type": "substring", "value": "ready" }
        }
    }
    "#;

    let request: Request = serde_json::from_str(json).unwrap();
    let Method::PaneWaitForOutput(params) = request.method else {
        panic!("wrong method parsed");
    };
    assert!(params.strip_ansi);
}

#[test]
/// Pane read defaults to text format.
fn omitted_pane_read_format_selects_text() {
    let json = r#"
    {
        "id": "req_1",
        "method": "pane.read",
        "params": {
            "pane_id": "p_1",
            "source": "visible"
        }
    }
    "#;

    let request: Request = serde_json::from_str(json).unwrap();
    let serialized = serde_json::to_value(&request).unwrap();
    assert!(serialized["params"].get("intent").is_none());
    let Method::PaneRead(params) = request.method else {
        panic!("wrong method parsed");
    };
    assert_eq!(params.format, ReadFormat::Text);
    assert_eq!(params.intent, ReadIntent::Interactive);
}

#[test]
/// Pane current request round trips.
fn current_pane_lookup_preserves_its_request_envelope() {
    let request = Request {
        id: "req_current".into(),
        method: Method::PaneCurrent(PaneCurrentParams {
            caller_pane_id: Some("w1-1".into()),
        }),
    };

    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["method"], "pane.current");
    assert_eq!(json["params"]["caller_pane_id"], "w1-1");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, request);
}

#[test]
/// Pane process info request round trips.
fn pane_process_lookup_preserves_its_request_envelope() {
    let request = Request {
        id: "req_process_info".into(),
        method: Method::PaneProcessInfo(PaneProcessInfoParams {
            pane_id: Some("w1-1".into()),
        }),
    };

    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["method"], "pane.process_info");
    assert_eq!(json["params"]["pane_id"], "w1-1");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, request);
}

#[test]
/// Event envelope round trips.
fn event_json_preserves_payload_and_envelope() {
    let events = [
        EventEnvelope {
            event: EventKind::PaneOutputChanged,
            data: EventData::PaneOutputChanged {
                pane_id: "p_1".into(),
                workspace_id: "w_1".into(),
                revision: 42,
            },
        },
        EventEnvelope {
            event: EventKind::WorkspaceMoved,
            data: EventData::WorkspaceMoved {
                workspace_id: "w_1".into(),
                insert_index: 2,
                workspaces: vec![],
            },
        },
        EventEnvelope {
            event: EventKind::WorkspaceReordered,
            data: EventData::WorkspaceReordered {
                workspace_ids: vec!["w_1".into(), "w_2".into()],
                before_workspace_id: Some("w_3".into()),
                workspaces: vec![],
            },
        },
        EventEnvelope {
            event: EventKind::TabMoved,
            data: EventData::TabMoved {
                tab_id: "w_1:1".into(),
                workspace_id: "w_1".into(),
                insert_index: 1,
                tabs: vec![],
            },
        },
        EventEnvelope {
            event: EventKind::LayoutUpdated,
            data: EventData::LayoutUpdated {
                layout: event_layout(),
            },
        },
    ];

    for event in events {
        let json = serde_json::to_string(&event).unwrap();
        let restored: EventEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, event);
    }
}

#[test]
/// Subscribe request parses parameterized subscriptions.
fn subscription_json_decodes_event_parameters() {
    let json = r#"
    {
        "id": "sub_1",
        "method": "events.subscribe",
        "params": {
            "subscriptions": [
                {
                    "type": "pane.output_matched",
                    "pane_id": "p_1_1",
                    "source": "recent",
                    "lines": 200,
                    "match": { "type": "substring", "value": "auth: received" }
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
        }
    }
    "#;

    let request: Request = serde_json::from_str(json).unwrap();
    let Method::EventsSubscribe(params) = request.method else {
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
    let event = SubscriptionEventEnvelope {
        event: SubscriptionEventKind::PaneOutputMatched,
        data: SubscriptionEventData::PaneOutputMatched(PaneOutputMatchedEvent {
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
        }),
    };

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("\"event\":\"pane.output_matched\""));
    let restored: SubscriptionEventEnvelope = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, event);
}

#[test]
/// Scroll changed subscription event round trips.
fn scroll_delivery_preserves_its_subscription_payload() {
    let event = SubscriptionEventEnvelope {
        event: SubscriptionEventKind::ScrollChanged,
        data: SubscriptionEventData::ScrollChanged(PaneScrollChangedEvent {
            pane_id: "p_1_1".into(),
            workspace_id: "w_1".into(),
            scroll: PaneScrollInfo {
                offset_from_bottom: 12,
                max_offset_from_bottom: 240,
                viewport_rows: 30,
            },
        }),
    };

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains("\"event\":\"pane.scroll_changed\""));
    let restored: SubscriptionEventEnvelope = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, event);
}

#[test]
/// Success response round trips.
fn successful_reply_survives_json_encoding() {
    let response = SuccessResponse {
        id: "req_1".into(),
        result: ResponseResult::Pong {
            version: "0.1.2".into(),
            protocol: 6,
            capabilities: Some(ServerCapabilities {
                live_handoff: true,
                detached_server_daemon: true,
            }),
        },
    };

    let json = serde_json::to_string(&response).unwrap();
    let restored: SuccessResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, response);
}

#[test]
/// Session snapshot request and response round trip.
fn session_snapshot_exchange_preserves_json_records() {
    let request = Request {
        id: "req_snapshot".into(),
        method: Method::SessionSnapshot(EmptyParams::default()),
    };
    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains("\"method\":\"session.snapshot\""));
    let restored: Request = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, request);

    let response = SuccessResponse {
        id: "req_snapshot".into(),
        result: ResponseResult::SessionSnapshot {
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
        },
    };
    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("\"type\":\"session_snapshot\""));
    let restored: SuccessResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, response);
}

#[test]
/// Worktree request and response round trip.
fn worktree_exchange_preserves_json_records() {
    let request = Request {
        id: "req_worktree".into(),
        method: Method::WorktreeCreate(WorktreeCreateParams {
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

    let response = SuccessResponse {
        id: "req_worktree".into(),
        result: ResponseResult::WorktreeCreated {
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
        },
    };
    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("\"type\":\"worktree_created\""));
    assert!(json.contains("\"worktree\""));
    let restored: SuccessResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, response);
}

#[test]
/// Worktree lifecycle events round trip.
fn worktree_notifications_preserve_lifecycle_payloads() {
    let subscription = Request {
        id: "sub_worktrees".into(),
        method: Method::EventsSubscribe(EventsSubscribeParams {
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
        EventEnvelope {
            event: EventKind::WorktreeCreated,
            data: EventData::WorktreeCreated {
                workspace: workspace.clone(),
                worktree: worktree.clone(),
            },
        },
        EventEnvelope {
            event: EventKind::WorktreeOpened,
            data: EventData::WorktreeOpened {
                workspace: workspace.clone(),
                worktree: worktree.clone(),
                already_open: false,
            },
        },
        EventEnvelope {
            event: EventKind::WorktreeRemoved,
            data: EventData::WorktreeRemoved {
                workspace_id: "w_2".into(),
                workspace: Some(workspace.clone()),
                worktree: WorktreeInfo {
                    open_workspace_id: None,
                    ..worktree
                },
                forced: false,
            },
        },
        EventEnvelope {
            event: EventKind::WorkspaceClosed,
            data: EventData::WorkspaceClosed {
                workspace_id: "w_2".into(),
                workspace: Some(workspace),
            },
        },
    ] {
        let json = serde_json::to_string(&event).unwrap();
        let restored: EventEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, event);
    }
}

#[test]
/// Plugin link list unlink round trip.
fn plugin_registration_commands_preserve_json_records() {
    let link = Request {
        id: "plugin_link".into(),
        method: Method::PluginLink(PluginLinkParams {
            path: "/plugins/worktree-bootstrap".into(),
            enabled: true,
            source: None,
        }),
    };
    let json = serde_json::to_string(&link).unwrap();
    assert!(json.contains("\"method\":\"plugin.link\""));
    let restored: Request = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, link);

    let list = Request {
        id: "plugin_list".into(),
        method: Method::PluginList(PluginListParams {
            plugin_id: Some("example.worktree-bootstrap".into()),
        }),
    };
    let json = serde_json::to_string(&list).unwrap();
    assert!(json.contains("\"method\":\"plugin.list\""));
    let restored: Request = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, list);

    let unlink = Request {
        id: "plugin_unlink".into(),
        method: Method::PluginUnlink(PluginUnlinkParams {
            plugin_id: "example.worktree-bootstrap".into(),
        }),
    };
    let json = serde_json::to_string(&unlink).unwrap();
    assert!(json.contains("\"method\":\"plugin.unlink\""));
    let restored: Request = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, unlink);

    let plugin = installed_plugin();

    for response in [
        SuccessResponse {
            id: "plugin_link".into(),
            result: ResponseResult::PluginLinked {
                plugin: plugin.clone(),
            },
        },
        SuccessResponse {
            id: "plugin_list".into(),
            result: ResponseResult::PluginList {
                plugins: vec![plugin.clone()],
            },
        },
        SuccessResponse {
            id: "plugin_unlink".into(),
            result: ResponseResult::PluginUnlinked {
                plugin_id: plugin.plugin_id,
                removed: true,
            },
        },
    ] {
        let json = serde_json::to_string(&response).unwrap();
        let restored: SuccessResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, response);
    }
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
        id: "layout_export".into(),
        method: Method::LayoutExport(LayoutExportParams {
            tab_id: Some("w1:1".into()),
            pane_id: None,
        }),
    };
    let json = serde_json::to_string(&export).unwrap();
    assert!(json.contains("\"method\":\"layout.export\""));
    let restored: Request = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, export);

    let apply = Request {
        id: "layout_apply".into(),
        method: Method::LayoutApply(LayoutApplyParams {
            workspace_id: Some("w1".into()),
            tab_id: None,
            tab_label: Some("dev".into()),
            focus: true,
            root: root.clone(),
        }),
    };
    let json = serde_json::to_string(&apply).unwrap();
    assert!(json.contains("\"method\":\"layout.apply\""));
    let restored: Request = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, apply);

    assert_layout_responses(root);
}

#[test]
/// Authority mutation requests round trip.
fn authority_commands_preserve_json_parameters() {
    let workspace_move = Request {
        id: "move_ws".into(),
        method: Method::WorkspaceMove(WorkspaceMoveParams {
            workspace_id: "w1".into(),
            insert_index: 2,
        }),
    };
    let json = serde_json::to_value(&workspace_move).unwrap();
    assert_eq!(json["method"], "workspace.move");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, workspace_move);

    let workspace_move_block = Request {
        id: "move_ws_block".into(),
        method: Method::WorkspaceMoveBlock(WorkspaceMoveBlockParams {
            workspace_ids: vec!["w1".into(), "w2".into()],
            before_workspace_id: Some("w3".into()),
        }),
    };
    let json = serde_json::to_value(&workspace_move_block).unwrap();
    assert_eq!(json["method"], "workspace.move_block");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, workspace_move_block);

    let tab_move = Request {
        id: "move_tab".into(),
        method: Method::TabMove(TabMoveParams {
            tab_id: "w1:1".into(),
            insert_index: 1,
        }),
    };
    let json = serde_json::to_value(&tab_move).unwrap();
    assert_eq!(json["method"], "tab.move");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, tab_move);

    assert_pane_authority_requests();
}

#[test]
/// Create response round trips with root pane.
fn creation_reply_preserves_the_root_pane() {
    let response = SuccessResponse {
        id: "req_2".into(),
        result: ResponseResult::TabCreated {
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
        },
    };

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("\"type\":\"tab_created\""));
    assert!(json.contains("\"root_pane\""));
    let restored: SuccessResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, response);
}

#[test]
/// Error response round trips.
fn failure_reply_preserves_error_fields() {
    let response = ErrorResponse {
        id: "req_1".into(),
        error: ErrorBody {
            code: "pane_not_found".into(),
            message: "pane p_1 not found".into(),
        },
    };

    let json = serde_json::to_string(&response).unwrap();
    let restored: ErrorResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, response);
}

#[test]
/// Event wait parses typed match.
fn event_wait_decodes_typed_matching_options() {
    let json = r#"
    {
        "id": "req_9",
        "method": "events.wait",
        "params": {
            "match_event": {
                "event": "pane_agent_status_changed",
                "pane_id": "p_1",
                "agent_status": "done"
            },
            "timeout_ms": 30000
        }
    }
    "#;

    let request: Request = serde_json::from_str(json).unwrap();
    let Method::EventsWait(params) = request.method else {
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
/// Plugin action list and invoke round trips.
fn plugin_action_exchange_preserves_json_records() {
    let list = Request {
        id: "req_plugin_action_list".into(),
        method: Method::PluginActionList(PluginActionListParams {
            plugin_id: Some("example.issue-flow".into()),
        }),
    };
    let json = serde_json::to_value(&list).unwrap();
    assert_eq!(json["method"], "plugin.action.list");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, list);

    let invoke = Request {
        id: "req_plugin_action_invoke".into(),
        method: Method::PluginActionInvoke(PluginActionInvokeParams {
            plugin_id: Some("example.issue-flow".into()),
            action_id: "assign-issue".into(),
            context: None,
        }),
    };
    let json = serde_json::to_value(&invoke).unwrap();
    assert_eq!(json["method"], "plugin.action.invoke");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, invoke);

    let action_info = PluginActionInfo {
        plugin_id: "example.issue-flow".into(),
        action_id: "assign-issue".into(),
        title: "Assign Issue".into(),
        description: Some("Open the issue assignment UI".into()),
        contexts: vec![PluginActionContext::Workspace, PluginActionContext::Pane],
        command: vec!["assign".into(), "--issue".into()],
        platforms: Some(vec![PluginPlatform::Linux, PluginPlatform::Macos]),
    };
    assert_eq!(
        action_info.qualified_id(),
        "example.issue-flow.assign-issue"
    );
    let json = serde_json::to_string(&action_info).unwrap();
    let restored: PluginActionInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, action_info);
}

#[test]
/// Plugin pane open request round trips.
fn plugin_pane_request_preserves_popup_options() {
    let request = Request {
        id: "req_plugin_pane".into(),
        method: Method::PluginPaneOpen(PluginPaneOpenParams {
            plugin_id: "example.board".into(),
            entrypoint: "board".into(),
            placement: Some(PluginPanePlacement::Popup),
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
    assert_eq!(json["method"], "plugin.pane.open");
    assert_eq!(json["params"]["placement"], "popup");
    assert_eq!(json["params"]["width"], 90);
    assert_eq!(json["params"]["height"], "80%");
    assert_eq!(json["params"]["env"]["MAESTRO_ROLE"], "board");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, request);
}

#[test]
/// Popup close emits its method name and empty parameters.
fn popup_close_serializes_the_method_and_empty_parameters() {
    let request = Request {
        id: "popup-close".into(),
        method: Method::PopupClose(EmptyParams::default()),
    };

    let json = serde_json::to_value(request).unwrap();

    assert_eq!(json["method"], "popup.close");
    assert_eq!(json["params"], serde_json::json!({}));
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
fn installed_plugin() -> InstalledPluginInfo {
    InstalledPluginInfo {
        plugin_id: "example.worktree-bootstrap".into(),
        name: "Worktree Bootstrap".into(),
        version: "0.1.0".into(),
        min_maestro_version: maestro_protocol::build_info::BASE_VERSION.into(),
        description: Some("Prepare new worktrees".into()),
        manifest_path: "/plugins/worktree-bootstrap/maestro-plugin.toml".into(),
        plugin_root: "/plugins/worktree-bootstrap".into(),
        enabled: true,
        platforms: None,
        build: vec![PluginManifestBuild {
            platforms: None,
            command: vec!["bun".into(), "install".into()],
        }],
        startup: vec![],
        actions: vec![PluginManifestAction {
            id: "bootstrap".into(),
            title: "Bootstrap worktree".into(),
            description: None,
            contexts: vec![PluginActionContext::Workspace],
            platforms: None,
            command: vec!["bun".into(), "run".into(), "bootstrap.ts".into()],
        }],
        events: vec![PluginManifestEventHook {
            on: "worktree.created".into(),
            platforms: None,
            command: vec!["bun".into(), "run".into(), "bootstrap.ts".into()],
        }],
        panes: vec![PluginManifestPane {
            id: "board".into(),
            title: "Board".into(),
            description: None,
            platforms: None,
            placement: PluginPanePlacement::Overlay,
            width: None,
            height: None,
            command: vec!["bun".into(), "run".into(), "board.ts".into()],
        }],
        link_handlers: vec![PluginManifestLinkHandler {
            id: "github-pr".into(),
            title: "Open GitHub PR".into(),
            pattern: "^https://github.com/[^/]+/[^/]+/(issues|pull)/[0-9]+$".into(),
            action: "bootstrap".into(),
            platforms: None,
        }],
        source: PluginSourceInfo::default(),
        warnings: vec![],
    }
}

/// Layout result envelopes.
fn assert_layout_responses(root: LayoutNode) {
    let response = SuccessResponse {
        id: "layout_export".into(),
        result: ResponseResult::LayoutExport {
            layout: LayoutDescription {
                workspace_id: "w1".into(),
                tab_id: "w1:1".into(),
                zoomed: false,
                focused_pane_id: "w1-1".into(),
                root,
            },
        },
    };
    let json = serde_json::to_string(&response).unwrap();
    let restored: SuccessResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, response);

    let response = SuccessResponse {
        id: "layout_ratio".into(),
        result: ResponseResult::LayoutSplitRatioSet {
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
        },
    };
    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("\"type\":\"layout_split_ratio_set\""));
    let restored: SuccessResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, response);
}

/// Pane authority and subscription request shapes.
fn assert_pane_authority_requests() {
    let pane_focus = Request {
        id: "focus_pane".into(),
        method: Method::PaneFocus(PaneTarget {
            pane_id: "w1:1".into(),
        }),
    };
    let json = serde_json::to_value(&pane_focus).unwrap();
    assert_eq!(json["method"], "pane.focus");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, pane_focus);

    let split_ratio = Request {
        id: "set_ratio".into(),
        method: Method::LayoutSetSplitRatio(LayoutSetSplitRatioParams {
            tab_id: Some("w1:1".into()),
            pane_id: None,
            path: vec![false, true],
            ratio: 0.6,
        }),
    };
    let json = serde_json::to_value(&split_ratio).unwrap();
    assert_eq!(json["method"], "layout.set_split_ratio");
    let restored: Request = serde_json::from_value(json).unwrap();
    assert_eq!(restored, split_ratio);

    let subscription = Request {
        id: "sub_moves".into(),
        method: Method::EventsSubscribe(EventsSubscribeParams {
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
