//! The daemon's workspace command handler and its arm in the daemon loop: server control,
//! notifications, workspace metadata and the one fallback.
use crate::locations::DaemonPaths;
use crate::service::RequestMessage;
use crate::settings::model::ToastDelivery;
use crate::state::App;
use crate::state::model::{ToastKind, ToastNotification};
use crate::state::test_support::api::{daemon, daemon_with_workspaces, failure, request, settings};
use crate::tests::block_on;
use maestro_protocol::rpc_types::RpcCommand;
use maestro_protocol::schema::{EmptyParams, NotificationShowReason, ResponseResult};
use maestro_protocol::shared::ToastMaestroPosition;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, oneshot};

#[test]
fn daemon_server_stop_request_sets_should_quit_flag() {
    let mut app = daemon().unwrap();
    assert_eq!(
        request(&mut app, json!({"type": "server.stop"})),
        Ok(ResponseResult::Ok {})
    );
    assert!(app.state.should_quit.0);

    // A stop arriving through the loop's request arm ends the run without its shutdown.
    let mut app = daemon().unwrap();
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
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
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
fn delivering(delivery: ToastDelivery) -> App {
    let mut app = daemon().unwrap();
    app.state.toast_config.delivery = delivery;
    app
}
/// Shows a notification titled `title` with `extra` members.
fn show(
    app: &mut App,
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
    let paths = crate::state::test_support::api::untouched_paths().unwrap();
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
        title: "Maestro needs attention".into(),
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
    let mut app = daemon().unwrap();
    let paths = crate::state::test_support::api::untouched_paths().unwrap();
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
    let defaults = crate::settings::model::Config::default();
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
