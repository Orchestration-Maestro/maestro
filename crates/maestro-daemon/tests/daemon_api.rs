//! The daemon's workspace command handler and its arm in the daemon loop: server control,
//! notifications, workspace metadata and the one fallback.
#![cfg(target_os = "linux")]
use crate::support::api::{
    TestApp, daemon, daemon_with_workspaces, events_after, failure, request, settings,
};
use crate::support::runtime::block_on;
use maestro_daemon::api::RequestMessage;
use maestro_daemon::app::state::{ToastKind, ToastNotification};
use maestro_daemon::paths::DaemonPaths;
use maestro_daemon::settings::model::ToastDelivery;
use maestro_protocol::events::EventData;
use maestro_protocol::rpc_types::RpcCommand;
use maestro_protocol::schema::{EmptyParams, NotificationShowReason, ResponseResult};
use maestro_protocol::shared::ToastMaestroPosition;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};

#[test]
fn daemon_server_stop_request_sets_should_quit_flag() {
    let mut app = daemon();
    assert_eq!(
        request(&mut app, json!({"type": "server.stop"})),
        Ok(ResponseResult::Ok {})
    );
    assert!(app.state.should_quit.0);

    // A stop arriving through the loop's request arm ends the run without its shutdown.
    let mut app = daemon();
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let paths = DaemonPaths::new(directory.path().to_str().unwrap(), None).unwrap();
    let (sender, mut requests) = mpsc::unbounded_channel();
    let (respond_to, mut response) = oneshot::channel();
    let (_attempted, response_write_complete) = oneshot::channel();
    sender
        .send(RequestMessage {
            command: RpcCommand::ServerStop(EmptyParams {}),
            respond_to,
            response_write_complete,
        })
        .unwrap();
    let mut settings = settings();
    block_on(app.run(&paths, &mut requests, &mut settings, std::future::pending()));
    assert_eq!(response.try_recv(), Ok(Ok(ResponseResult::Ok {})));
    assert!(app.state.should_quit.0);
}

#[test]
fn undelivered_families_use_one_source_fallback() {
    let mut app = daemon_with_workspaces(&["/one"]);
    for command in [
        json!({"type": "server.live_handoff"}),
        json!({"type": "client.window_title.clear"}),
        json!({"type": "session.snapshot"}),
        json!({"type": "worktree.list"}),
        json!({"type": "agent.list"}),
        json!({"type": "events.wait", "match_event": {"event": "workspace_created"}}),
        json!({"type": "integration.install", "target": "pi"}),
        json!({"type": "action.list"}),
    ] {
        assert_eq!(
            request(&mut app, command.clone()),
            failure("not_implemented", "method not implemented yet"),
            "{command}"
        );
    }
    assert!(!app.state.should_quit.0);
}

/// A daemon delivering notifications through `delivery`.
fn delivering(delivery: ToastDelivery) -> TestApp {
    let mut app = daemon();
    app.state.toast_config.delivery = delivery;
    app
}

/// Shows a notification titled `title` with `extra` members.
fn show(
    app: &mut TestApp,
    title: &str,
    extra: &Value,
) -> Result<ResponseResult, maestro_protocol::schema::ErrorBody> {
    let mut command = json!({"type": "notification.show", "title": title});
    if let (Some(command), Some(extra)) = (command.as_object_mut(), extra.as_object()) {
        command.extend(extra.clone());
    }
    request(app, command)
}

/// The result of a notification request with `reason`.
fn notified(reason: NotificationShowReason) -> ResponseResult {
    ResponseResult::NotificationShow {
        shown: reason == NotificationShowReason::Shown,
        reason,
    }
}

#[test]
fn notification_request_creates_workspace_toast_with_position() {
    let mut app = delivering(ToastDelivery::Workspace);
    let extra = json!({"body": "api workspace", "position": "top-left", "sound": "done"});
    assert_eq!(
        show(&mut app, "build failed", &extra),
        Ok(notified(NotificationShowReason::Shown))
    );
    assert_eq!(
        app.state.toast,
        Some(ToastNotification {
            kind: ToastKind::UpdateInstalled,
            title: "build failed".into(),
            context: "api workspace".into(),
            position: Some(ToastMaestroPosition::TopLeft),
            target: None,
        })
    );
    assert!(app.toast_deadline.is_some());
}

#[test]
fn notification_request_workspace_toast_expires() {
    let mut app = delivering(ToastDelivery::Workspace);
    let paths = crate::support::api::untouched_paths();
    assert_eq!(
        show(&mut app, "build failed", &json!({})),
        Ok(notified(NotificationShowReason::Shown))
    );
    let deadline = app.toast_deadline.unwrap();
    assert_eq!(
        app.next_headless_loop_deadline(Instant::now(), false),
        Some(deadline)
    );
    block_on(app.handle_scheduled_tasks(
        deadline.checked_sub(Duration::from_millis(1)).unwrap(),
        &paths,
    ));
    assert!(app.state.toast.is_some());
    assert!(block_on(app.handle_scheduled_tasks(deadline, &paths)));
    assert!(app.state.toast.is_none());
    assert!(app.toast_deadline.is_none());
}

#[test]
fn notification_request_respects_off_delivery() {
    let mut app = delivering(ToastDelivery::Off);
    assert_eq!(
        show(&mut app, "build failed", &json!({"sound": "request"})),
        Ok(notified(NotificationShowReason::Disabled))
    );
    assert!(app.state.toast.is_none());
    assert!(app.last_api_notification_at.is_none());
    assert!(app.toast_deadline.is_none());
}

#[test]
fn notification_request_does_not_replace_existing_toast() {
    let mut app = delivering(ToastDelivery::Workspace);
    let retained = ToastNotification {
        kind: ToastKind::NeedsAttention,
        title: "pi needs attention".into(),
        context: "background · 2".into(),
        position: Some(ToastMaestroPosition::BottomLeft),
        target: None,
    };
    app.state.toast = Some(retained.clone());
    let deadline = Instant::now() + Duration::from_secs(8);
    app.toast_deadline = Some(deadline);
    assert_eq!(
        show(&mut app, "build failed", &json!({"position": "top-right"})),
        Ok(notified(NotificationShowReason::Busy))
    );
    assert_eq!(app.state.toast, Some(retained));
    assert_eq!(app.toast_deadline, Some(deadline));
    assert!(app.last_api_notification_at.is_none());
}

#[test]
fn notification_request_is_rate_limited() {
    let mut app = delivering(ToastDelivery::Workspace);
    assert_eq!(
        show(&mut app, "first", &json!({})),
        Ok(notified(NotificationShowReason::Shown))
    );
    let shown = app.last_api_notification_at.unwrap();
    app.state.toast = None;
    assert_eq!(
        show(&mut app, "second", &json!({})),
        Ok(notified(NotificationShowReason::RateLimited))
    );
    assert!(app.state.toast.is_none());
    assert_eq!(app.last_api_notification_at, Some(shown));
    // Half a second later the allowance is still spent; a full second later it is back.
    app.last_api_notification_at = Some(
        Instant::now()
            .checked_sub(Duration::from_millis(500))
            .unwrap(),
    );
    assert_eq!(
        show(&mut app, "third", &json!({})),
        Ok(notified(NotificationShowReason::RateLimited))
    );
    app.last_api_notification_at =
        Some(Instant::now().checked_sub(Duration::from_secs(1)).unwrap());
    assert_eq!(
        show(&mut app, "fourth", &json!({})),
        Ok(notified(NotificationShowReason::Shown))
    );
    assert_eq!(
        app.state.toast.as_ref().map(|toast| toast.title.as_str()),
        Some("fourth")
    );
}

/// The title and context a shown workspace toast received for `title` and `body`.
fn shown_text(title: &str, body: Option<&str>) -> (String, String) {
    let mut app = delivering(ToastDelivery::Workspace);
    let extra = body.map_or_else(|| json!({}), |body| json!({"body": body}));
    assert_eq!(
        show(&mut app, title, &extra),
        Ok(notified(NotificationShowReason::Shown))
    );
    let toast = app.state.toast.unwrap();
    (toast.title, toast.context)
}

#[test]
fn notification_text_uses_scalar_and_whitespace_boundaries() {
    for title in ["", " \t\r\n ", "\u{0}\u{7}", "\u{85}"] {
        let mut app = delivering(ToastDelivery::Workspace);
        assert_eq!(
            show(&mut app, title, &json!({})),
            failure("invalid_params", "notification title is empty"),
            "{title:?}"
        );
        assert!(app.state.toast.is_none());
    }
    let off = &mut delivering(ToastDelivery::Off);
    assert_eq!(
        show(off, "\u{7}", &json!({})),
        failure("invalid_params", "notification title is empty")
    );
    let eighty = "é".repeat(80);
    assert_eq!(shown_text(&eighty, None).0, eighty);
    assert_eq!(shown_text(&format!("{eighty}x"), None).0, eighty);
    // Retained whitespace counts toward the limit before the final trim.
    assert_eq!(
        shown_text(&format!("{} b", "a".repeat(79)), None).0,
        "a".repeat(79)
    );
    let body = "β".repeat(240);
    assert_eq!(shown_text("t", Some(&body)).1, body);
    assert_eq!(shown_text("t", Some(&format!("{body}z"))).1, body);
    assert_eq!(shown_text("t", None).1, "");
    assert_eq!(shown_text("t", Some("\u{0}\u{1b}")).1, "");
    for (raw, clean) in [
        ("a\u{0}b", "ab"),
        ("a\r\nb\tc", "a b c"),
        ("  a   \u{2003}\u{a0} b  ", "a b"),
        ("a\u{85}b", "ab"),
        ("a\u{a0}b", "a b"),
        ("\u{feff}a\u{feff}", "\u{feff}a\u{feff}"),
    ] {
        assert_eq!(
            shown_text(raw, Some(raw)),
            (clean.into(), clean.into()),
            "{raw:?}"
        );
    }
}

#[test]
fn notification_without_a_foreground_client_preserves_policy() {
    for delivery in [ToastDelivery::Terminal, ToastDelivery::System] {
        let mut app = delivering(delivery);
        for sound in ["none", "done", "request"] {
            assert_eq!(
                show(&mut app, "build", &json!({"sound": sound})),
                Ok(notified(NotificationShowReason::NoForegroundClient)),
                "{delivery:?} {sound}"
            );
        }
        assert!(app.last_api_notification_at.is_none());
        assert!(app.state.toast.is_none());
        // A retained toast does not make them busy; a recent delivery still limits them.
        app.state.toast = Some(ToastNotification {
            kind: ToastKind::Finished,
            title: "done".into(),
            context: String::new(),
            position: None,
            target: None,
        });
        assert_eq!(
            show(&mut app, "build", &json!({})),
            Ok(notified(NotificationShowReason::NoForegroundClient))
        );
        app.last_api_notification_at = Some(Instant::now());
        assert_eq!(
            show(&mut app, "build", &json!({})),
            Ok(notified(NotificationShowReason::RateLimited))
        );
    }
    // A workspace toast delivered just before still limits a later terminal delivery.
    let mut app = delivering(ToastDelivery::Workspace);
    assert_eq!(
        show(&mut app, "first", &json!({})),
        Ok(notified(NotificationShowReason::Shown))
    );
    app.state.toast_config.delivery = ToastDelivery::Terminal;
    assert_eq!(
        show(&mut app, "second", &json!({})),
        Ok(notified(NotificationShowReason::RateLimited))
    );
}

/// Reports `tokens` from `source` for workspace `w1`, with `extra` members.
fn report(
    app: &mut TestApp,
    source: &str,
    tokens: &Value,
    extra: &Value,
) -> Result<ResponseResult, maestro_protocol::schema::ErrorBody> {
    let mut command = json!({
        "type": "workspace.report_metadata",
        "workspace_id": "w1",
        "source": source,
        "tokens": tokens,
    });
    if let (Some(command), Some(extra)) = (command.as_object_mut(), extra.as_object()) {
        command.extend(extra.clone());
    }
    request(app, command)
}

/// The tokens workspace `w1` currently holds.
fn tokens(app: &TestApp) -> HashMap<String, String> {
    app.state.topology.workspaces[0].metadata_tokens.values()
}

/// The token values of `pairs`.
fn values(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect()
}

/// The tokens each metadata event since `sequence` published.
fn published(app: &TestApp, sequence: u64) -> Vec<HashMap<String, String>> {
    events_after(app, sequence)
        .into_iter()
        .map(|event| match event {
            EventData::WorkspaceMetadataUpdated { workspace } => workspace.tokens,
            other => panic!("unexpected event {other:?}"),
        })
        .collect()
}

#[test]
fn workspace_tokens_patch_clear_and_publish_current_values() {
    let mut app = daemon_with_workspaces(&["/one"]);
    let first = json!({"summary": "reviewing auth", "jj_status": "2 changes"});
    assert_eq!(
        report(&mut app, "user:test", &first, &json!({})),
        Ok(ResponseResult::Ok {})
    );
    let expected = values(&[("summary", "reviewing auth"), ("jj_status", "2 changes")]);
    assert_eq!(tokens(&app), expected);
    let second = json!({"summary": "done", "jj_status": null});
    assert_eq!(
        report(&mut app, "user:test", &second, &json!({})),
        Ok(ResponseResult::Ok {})
    );
    assert_eq!(tokens(&app), values(&[("summary", "done")]));
    // Reporting the same values again changes nothing and publishes nothing.
    let same = json!({"summary": "done"});
    assert_eq!(
        report(&mut app, "user:test", &same, &json!({})),
        Ok(ResponseResult::Ok {})
    );
    let cleared = json!({"summary": "  "});
    assert_eq!(
        report(&mut app, "user:test", &cleared, &json!({})),
        Ok(ResponseResult::Ok {})
    );
    assert!(tokens(&app).is_empty());
    assert_eq!(
        published(&app, 0),
        [expected, values(&[("summary", "done")]), HashMap::new()]
    );
}

#[test]
fn workspace_token_expiry_publishes_the_remaining_snapshot() {
    let mut app = daemon_with_workspaces(&["/one"]);
    let paths = crate::support::api::untouched_paths();
    let lasting = json!({"lasting": "kept"});
    assert!(report(&mut app, "user:keep", &lasting, &json!({})).is_ok());
    let before = Instant::now();
    let temporary = json!({"summary": "temporary"});
    assert!(
        report(
            &mut app,
            "user:test",
            &temporary,
            &json!({"ttl_ms": 60_000})
        )
        .is_ok()
    );
    let deadline = app
        .next_headless_loop_deadline(Instant::now(), false)
        .unwrap();
    assert!(deadline >= before + Duration::from_secs(60));
    let sequence = app.event_hub.current_sequence();
    block_on(app.handle_scheduled_tasks(
        deadline.checked_sub(Duration::from_nanos(1)).unwrap(),
        &paths,
    ));
    assert_eq!(
        tokens(&app),
        values(&[("lasting", "kept"), ("summary", "temporary")])
    );
    block_on(app.handle_scheduled_tasks(deadline, &paths));
    assert_eq!(tokens(&app), values(&[("lasting", "kept")]));
    block_on(app.handle_scheduled_tasks(deadline + Duration::from_secs(1), &paths));
    assert_eq!(published(&app, sequence), [values(&[("lasting", "kept")])]);
}

#[test]
fn token_values_strip_controls_and_empty_values_clear() {
    let mut app = daemon_with_workspaces(&["/one"]);
    let set = json!({
        "summary": "  review\nready  ",
        "padded": "\u{85} a\u{0}b \u{85}",
        "long": "é".repeat(81),
        "controlled": format!(" \u{1b}{} ", "x".repeat(80)),
        "kept": "keep",
    });
    assert!(report(&mut app, "user:test", &set, &json!({})).is_ok());
    let expected = values(&[
        ("summary", "reviewready"),
        ("padded", "ab"),
        ("long", &"é".repeat(80)),
        ("controlled", &"x".repeat(80)),
        ("kept", "keep"),
    ]);
    assert_eq!(tokens(&app), expected);
    let clear = json!({"summary": null, "padded": " \n ", "long": "\u{0}\u{7}", "controlled": ""});
    assert!(report(&mut app, "user:test", &clear, &json!({})).is_ok());
    assert_eq!(tokens(&app), values(&[("kept", "keep")]));
}

#[test]
fn token_reports_reject_invalid_keys_and_excess_updates() {
    let mut app = daemon_with_workspaces(&["/one"]);
    let invalid = |message: &str| failure("invalid_metadata_token", message);
    let too_many: serde_json::Map<String, Value> = (0..17)
        .map(|index| (format!("key{index}"), json!("value")))
        .collect();
    let sixteen: serde_json::Map<String, Value> = (0..16)
        .map(|index| (format!("key{index}"), json!("value")))
        .collect();
    let long_key = "x".repeat(33);
    for (patch, expected) in [
        (json!({}), invalid("missing token to set or clear")),
        (
            Value::Object(too_many),
            invalid("a metadata report may update at most 16 tokens"),
        ),
        (
            json!({"": "value"}),
            invalid("invalid metadata token key: "),
        ),
        (
            json!({"bad.name": "value"}),
            invalid("invalid metadata token key: bad.name"),
        ),
        (
            json!({"clé": "value"}),
            invalid("invalid metadata token key: clé"),
        ),
        (
            json!({&long_key: "value"}),
            invalid(&format!("invalid metadata token key: {long_key}")),
        ),
    ] {
        assert_eq!(
            report(&mut app, "user:test", &patch, &json!({"seq": 5})),
            expected,
            "{patch}"
        );
    }
    assert!(tokens(&app).is_empty());
    assert!(
        app.state.topology.workspaces[0]
            .metadata_token_sequences
            .is_empty()
    );
    assert!(events_after(&app, 0).is_empty());
    let accepted = json!({"x".repeat(32): "value", "ok_key-2": "v"});
    assert!(report(&mut app, "user:test", &accepted, &json!({})).is_ok());
    assert!(report(&mut app, "user:test", &Value::Object(sixteen), &json!({})).is_ok());
    assert_eq!(tokens(&app).len(), 18);
}

#[test]
fn metadata_source_and_ttl_boundaries_keep_error_order() {
    let mut app = daemon_with_workspaces(&["/one"]);
    let token = json!({"summary": "x"});
    let source = |message: &str| failure("invalid_metadata_source", message);
    let ttl = |message: &str| failure("invalid_metadata_ttl", message);
    let ascii = "metadata source may contain only ASCII letters, digits, colon, dot, underscore, and hyphen";
    for (reporter, extra, expected) in [
        ("", json!({}), source("metadata source must not be empty")),
        (
            " \u{85} ",
            json!({}),
            source("metadata source must not be empty"),
        ),
        (
            "s".repeat(81).as_str(),
            json!({}),
            source("metadata source must be 80 characters or fewer"),
        ),
        ("user/test", json!({}), source(ascii)),
        ("usér", json!({}), source(ascii)),
        ("\u{feff}user", json!({}), source(ascii)),
        (
            "user:test",
            json!({"ttl_ms": 0}),
            ttl("metadata ttl_ms must be at least 1"),
        ),
        (
            "user:test",
            json!({"ttl_ms": 86_400_001}),
            ttl("metadata ttl_ms must be 86400000 or less"),
        ),
        // The source is checked before the lifetime, the lifetime before the tokens.
        (
            "",
            json!({"ttl_ms": 0}),
            source("metadata source must not be empty"),
        ),
    ] {
        assert_eq!(
            report(&mut app, reporter, &token, &extra),
            expected,
            "{reporter:?} {extra}"
        );
    }
    assert_eq!(
        report(&mut app, "user:test", &json!({}), &json!({"ttl_ms": 0})),
        ttl("metadata ttl_ms must be at least 1")
    );
    // An unknown workspace wins over every other fault.
    let mut missing = json!({"type": "workspace.report_metadata", "workspace_id": "w9", "source": "", "tokens": {}});
    assert_eq!(
        request(&mut app, missing.clone()),
        failure("workspace_not_found", "workspace w9 not found")
    );
    missing["workspace_id"] = json!("w_9");
    assert_eq!(
        request(&mut app, missing),
        source("metadata source must not be empty")
    );
    assert!(tokens(&app).is_empty());
    assert_boundary_sources_and_lifetimes_apply(&mut app, &token);
}

/// Sources and lifetimes on the accepted side of each boundary apply without tracking a
/// sequence.
fn assert_boundary_sources_and_lifetimes_apply(app: &mut TestApp, token: &Value) {
    for (reporter, extra) in [
        ("s".repeat(80).as_str(), json!({})),
        ("\u{85}a:b.c_d-e\u{85}", json!({"ttl_ms": null})),
        ("user:test", json!({"ttl_ms": 1})),
        ("user:test", json!({"ttl_ms": 86_400_000})),
    ] {
        assert_eq!(
            report(app, reporter, token, &extra),
            Ok(ResponseResult::Ok {}),
            "{reporter:?}"
        );
    }
    assert!(
        app.state.topology.workspaces[0]
            .metadata_token_sequences
            .is_empty()
    );
}

#[test]
fn metadata_capacity_and_sequences_are_transactional() {
    let mut app = daemon_with_workspaces(&["/one"]);
    let full: serde_json::Map<String, Value> = (0..16)
        .map(|index| (format!("a{index}"), json!("v")))
        .collect();
    let more: serde_json::Map<String, Value> = (0..16)
        .map(|index| (format!("b{index}"), json!("v")))
        .collect();
    assert!(report(&mut app, "user:a", &Value::Object(full), &json!({})).is_ok());
    assert!(report(&mut app, "user:b", &Value::Object(more), &json!({})).is_ok());
    assert_eq!(tokens(&app).len(), 32);
    let limit = failure(
        "metadata_token_limit",
        "workspace metadata may contain at most 32 tokens",
    );
    let sequence = app.event_hub.current_sequence();
    assert_eq!(
        report(&mut app, "user:c", &json!({"c": "v"}), &json!({"seq": 1})),
        limit
    );
    assert!(
        app.state.topology.workspaces[0]
            .metadata_token_sequences
            .is_empty()
    );
    // A patch that replaces or clears a retained key stays within capacity.
    assert!(
        report(
            &mut app,
            "user:c",
            &json!({"a0": "new", "c": "v", "a1": null}),
            &json!({"seq": 2})
        )
        .is_ok()
    );
    assert_eq!(tokens(&app).len(), 32);
    // Sequences: a stale or repeated report changes nothing; a newer one applies.
    for (seq, value, applied) in [(2, "stale", false), (1, "older", false), (3, "newer", true)] {
        assert_eq!(
            report(
                &mut app,
                "user:c",
                &json!({"c": value}),
                &json!({"seq": seq})
            ),
            Ok(ResponseResult::Ok {})
        );
        assert_eq!(tokens(&app)["c"] == value, applied, "{seq}");
    }
    assert_eq!(
        app.state.topology.workspaces[0].metadata_token_sequences["user:c"],
        3
    );
    // A stale report is accepted without effect even when it would exceed capacity.
    assert_eq!(
        report(&mut app, "user:c", &json!({"z": "v"}), &json!({"seq": 1})),
        Ok(ResponseResult::Ok {})
    );
    assert!(!tokens(&app).contains_key("z"));
    assert_eq!(published(&app, sequence).len(), 2);
    assert_sequence_source_capacity();
}

/// Thirty-two sequenced sources are tracked; a thirty-third new one is refused, while an
/// unsequenced report from it still applies.
fn assert_sequence_source_capacity() {
    let mut app = daemon_with_workspaces(&["/one"]);
    for index in 0..32 {
        assert!(
            report(
                &mut app,
                &format!("s{index}"),
                &json!({"k": format!("{index}")}),
                &json!({"seq": 1})
            )
            .is_ok()
        );
    }
    let sources = failure(
        "metadata_sequence_source_limit",
        "workspace metadata may track at most 32 sequenced sources",
    );
    assert_eq!(
        report(&mut app, "s32", &json!({"k": "late"}), &json!({"seq": 1})),
        sources
    );
    assert_eq!(tokens(&app)["k"], "31");
    assert!(report(&mut app, "s32", &json!({"k": "unsequenced"}), &json!({})).is_ok());
    assert_eq!(
        app.state.topology.workspaces[0]
            .metadata_token_sequences
            .len(),
        32
    );
}

#[test]
fn configuration_reload_uses_the_retained_settings_owner() {
    use maestro_settings::{
        InMemorySettingsStorage, SettingsManager, SettingsScope, SettingsStorage,
    };
    let storage = std::sync::Arc::new(InMemorySettingsStorage::new());
    let document = json!({
        "multiplexer": {"toast": {"delivery": "workspace"}},
        "daemon": {"headlessCols": 0, "headlessRows": 0},
    });
    storage
        .with_lock(SettingsScope::Global, &mut |_| {
            Ok(Some(document.to_string()))
        })
        .unwrap();
    let mut manager = SettingsManager::from_storage(storage);
    let mut app = daemon();
    let paths = crate::support::api::untouched_paths();
    let reload = RpcCommand::ServerReloadConfig(EmptyParams {});
    let result = block_on(app.handle_api_request(reload, &mut manager, &paths));
    let Ok(ResponseResult::ConfigReload {
        status,
        diagnostics,
    }) = result
    else {
        panic!("expected a reload report: {result:?}")
    };
    assert_eq!(
        status,
        maestro_protocol::shared::ConfigReloadStatus::Partial
    );
    assert!(
        diagnostics
            .iter()
            .any(|message| message.contains("daemon.headlessCols"))
    );
    assert_eq!(app.state.toast_config.delivery, ToastDelivery::Workspace);
    let defaults = maestro_daemon::settings::model::Config::default();
    assert_eq!(app.state.headless_size, defaults.headless_size());
    // The accepted delivery now governs a later notification request.
    assert_eq!(
        request(
            &mut app,
            json!({"type": "notification.show", "title": "after reload"})
        ),
        Ok(ResponseResult::NotificationShow {
            shown: false,
            reason: NotificationShowReason::Busy
        })
    );
    assert_eq!(
        app.state.toast.as_ref().map(|toast| toast.title.as_str()),
        Some("reloaded config")
    );
}
