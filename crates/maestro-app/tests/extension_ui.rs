//! The extension interface over the protocol: dialogs answered by the client, notices
//! nobody answers and the headless defaults of terminal-only operations.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::export_fixtures::published;
use crate::support::rpc_connection::{
    Client, Outcomes, connect, no_workspace, poll_once, serve, startup_select,
};
use crate::support::runtime_fixtures::runtime_for;
use crate::support::session_harness::{Harness, on_paused_task_set, on_task_set};
use maestro_app::{
    AgentSessionRuntime, Extension, ExtensionAPI, Local, SourceInfo, SourceOrigin, SourceScope,
};
use maestro_extensions::types::{
    AutocompleteProviderFactory, ComponentFactory, CustomComponentFactory, EditorFactory,
    ExtensionUIContext, ExtensionUIDialogOptions, ExtensionWidgetOptions, FooterFactory,
    NotificationType, ThemeSelection, WidgetContent, WidgetPlacement, WorkingIndicatorOptions,
};
use maestro_models::Cancellation;
use maestro_theme::{Theme, ThemeState};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    pin::pin,
    rc::Rc,
    sync::{Arc, RwLock},
    task::Poll,
    time::Duration,
};

/// A runtime bound to its connection, the client and the protocol's extension interface.
async fn bound(label: &str) -> (Harness, Rc<AgentSessionRuntime>, Client, ExtensionUIContext) {
    let (h, runtime, _) = runtime_for(label, Vec::new()).await;
    let runtime = Rc::new(runtime);
    let mut client = connect(Rc::clone(&runtime));
    // The first answer follows the initial binding.
    client.ask(r#"{"type":"get_state"}"#).await;
    let ui = runtime.session().extension_runner().get_ui_context();
    (h, runtime, client, ui)
}

/// A facade whose UI callbacks are handed to the protocol's interface.
fn facade() -> ExtensionAPI<Local> {
    let source = Arc::new(RwLock::new(SourceInfo {
        path: "test".into(),
        source: "test".into(),
        scope: SourceScope::Temporary,
        origin: SourceOrigin::TopLevel,
        base_dir: None,
    }));
    let owner = Rc::new(RwLock::new(Extension::<Local>::new(
        "test".into(),
        "test".into(),
        source,
    )));
    let events = maestro_app::create_event_bus_for::<Local>(Rc::new(|_| {}), Rc::new(|_| {}))
        .for_extension();
    ExtensionAPI::new(
        &owner,
        maestro_app::create_extension_runtime::<Local>(),
        events,
        ".".into(),
    )
}

/// `request` without its identifier, after checking that the identifier is a fresh UUID v4.
fn without_fresh_id(mut request: Value, seen: &mut HashSet<String>) -> Value {
    let id = request["id"].as_str().unwrap().to_owned();
    let parsed = uuid::Uuid::parse_str(&id).unwrap();
    assert_eq!(parsed.get_version_num(), 4, "{id}");
    assert_eq!(parsed.hyphenated().to_string(), id);
    assert!(seen.insert(id), "a fresh identifier per request");
    request.as_object_mut().unwrap().shift_remove("id");
    request
}

/// Answers the dialog `request` with the response members `answer`.
fn respond(client: &Client, request: &Value, answer: &Value) {
    let mut response = json!({"type": "extension_ui_response", "id": request["id"]});
    response
        .as_object_mut()
        .unwrap()
        .extend(answer.as_object().unwrap().clone());
    client.send(&response.to_string());
}

/// Checks that nothing was written since the last record read.
async fn nothing_written(client: &mut Client) {
    let next = client.ask(r#"{"id":"quiet","type":"get_state"}"#).await;
    assert_eq!(
        next["id"], "quiet",
        "no record came before the probe's answer"
    );
}

/// Every method writes its request as soon as it is called, each with a fresh UUID v4; a
/// response sent as soon as the request is read settles the dialog.
#[test]
fn dialogs_register_before_emission() {
    on_task_set(|_| async {
        let (_h, _runtime, mut client, ui) = bound("emitted").await;
        let mut seen = HashSet::new();
        let select = ui.select("pick".into(), vec!["a".into(), "b".into()], None);
        let request = client.next().await;
        respond(&client, &request, &json!({"value": "a"}));
        assert_eq!(select.await.unwrap().as_deref(), Some("a"));
        assert_eq!(
            without_fresh_id(request, &mut seen),
            json!({"type": "extension_ui_request", "method": "select", "title": "pick",
                "options": ["a", "b"]})
        );
        let confirm = ui.confirm("sure".into(), "really".into(), None);
        let request = client.next().await;
        respond(&client, &request, &json!({"confirmed": true}));
        assert!(confirm.await.unwrap());
        assert_eq!(
            without_fresh_id(request, &mut seen),
            json!({"type": "extension_ui_request", "method": "confirm", "title": "sure",
                "message": "really"})
        );
        let input = ui.input("name".into(), Some("prefill".into()), None);
        let request = client.next().await;
        respond(&client, &request, &json!({"value": "typed"}));
        assert_eq!(input.await.unwrap().as_deref(), Some("typed"));
        assert_eq!(
            without_fresh_id(request, &mut seen),
            json!({"type": "extension_ui_request", "method": "input", "title": "name",
                "placeholder": "prefill"})
        );
        let editor = ui.editor("edit".into(), Some("draft".into()));
        let request = client.next().await;
        respond(&client, &request, &json!({"value": "edited"}));
        assert_eq!(editor.await.unwrap().as_deref(), Some("edited"));
        assert_eq!(
            without_fresh_id(request, &mut seen),
            json!({"type": "extension_ui_request", "method": "editor", "title": "edit",
                "prefill": "draft"})
        );
        ui.notify("note".into(), None).unwrap();
        ui.set_status("key".into(), Some("text".into())).unwrap();
        ui.set_widget("key".into(), None, None).unwrap();
        ui.set_title("title".into()).unwrap();
        ui.set_editor_text("text".into()).unwrap();
        for method in [
            "notify",
            "setStatus",
            "setWidget",
            "setTitle",
            "set_editor_text",
        ] {
            let request = without_fresh_id(client.next().await, &mut seen);
            assert_eq!(request["method"], method);
        }
        assert_eq!(seen.len(), 9);
    });
}

/// Values and confirmations settle their dialog, a dismissal wins over supplied members,
/// and a response to a settled, unknown or malformed identifier is ignored.
#[test]
fn dialog_cancellation_wins_and_cleans_routes() {
    on_task_set(|_| async {
        let (_h, _runtime, mut client, ui) = bound("answered").await;
        let options = || vec!["a".to_owned(), "b".to_owned()];
        for (answer, expected) in [
            (json!({"value": "answer"}), Some("answer")),
            (json!({"cancelled": true, "value": "answer"}), None),
            (json!({}), None),
            (
                json!({"value": "a", "confirmed": true, "cancelled": false}),
                Some("a"),
            ),
            (json!({"value": ""}), Some("")),
        ] {
            let select = ui.select("title".into(), options(), None);
            let request = client.next().await;
            respond(&client, &request, &answer);
            assert_eq!(select.await.unwrap().as_deref(), expected, "{answer}");
            respond(&client, &request, &json!({"value": "duplicate"}));
            nothing_written(&mut client).await;
        }
        for (answer, expected) in [
            (json!({"confirmed": true}), true),
            (json!({"confirmed": false}), false),
            (json!({"cancelled": true, "confirmed": true}), false),
            (json!({}), false),
            (
                json!({"value": "ignored", "confirmed": true, "cancelled": false}),
                true,
            ),
        ] {
            let confirm = ui.confirm("title".into(), "message".into(), None);
            let request = client.next().await;
            respond(&client, &request, &answer);
            assert_eq!(confirm.await.unwrap(), expected, "{answer}");
        }
        let editor = ui.editor("title".into(), None);
        let request = client.next().await;
        respond(&client, &request, &json!({"confirmed": true}));
        assert_eq!(editor.await.unwrap(), None);
        let select = ui.select("bad id".into(), vec!["a".into()], None);
        let request = client.next().await;
        for id in [
            None,
            Some(json!(null)),
            Some(json!(0)),
            Some(json!(false)),
            Some(json!({})),
            Some(json!([])),
            Some(json!("unknown")),
        ] {
            let mut response = json!({"type": "extension_ui_response", "value": "ignored"});
            if let Some(id) = id {
                response["id"] = id;
            }
            client.send(&response.to_string());
        }
        nothing_written(&mut client).await;
        respond(&client, &request, &json!({"value": "a"}));
        assert_eq!(select.await.unwrap().as_deref(), Some("a"));
    });
}

/// A dialog whose signal already aborted settles with its default and writes nothing.
#[test]
fn preaborted_dialogs_emit_nothing() {
    on_task_set(|_| async {
        let (_h, _runtime, mut client, ui) = bound("preaborted").await;
        let aborted = || {
            let signal = Cancellation::new();
            signal.abort();
            Some(ExtensionUIDialogOptions {
                signal: Some(signal),
                timeout: None,
            })
        };
        assert_eq!(
            ui.select("t".into(), vec!["a".into()], aborted())
                .await
                .unwrap(),
            None
        );
        assert!(!ui.confirm("t".into(), "m".into(), aborted()).await.unwrap());
        assert_eq!(ui.input("t".into(), None, aborted()).await.unwrap(), None);
        nothing_written(&mut client).await;
        let signal = Cancellation::new();
        let options = ExtensionUIDialogOptions {
            signal: Some(signal.clone()),
            timeout: None,
        };
        let select = ui.select("t".into(), vec!["a".into()], Some(options));
        client.next().await;
        signal.abort();
        assert_eq!(
            select.await.unwrap(),
            None,
            "a later abort settles the default"
        );
    });
}

/// Without a timeout or with zero a dialog waits; a positive timeout settles the default
/// exactly at its boundary and is written with the request.
#[test]
fn dialog_deadline_keeps_zero_and_exact_boundary() {
    on_paused_task_set(|_| async {
        let (_h, _runtime, mut client, ui) = bound("deadline").await;
        let timed = |timeout| {
            Some(ExtensionUIDialogOptions {
                signal: None,
                timeout,
            })
        };
        let mut waiting = pin!(ui.select("t".into(), vec!["a".into()], timed(None)));
        assert!(client.next().await.get("timeout").is_none());
        let mut zero = pin!(ui.confirm("t".into(), "m".into(), timed(Some(0.0))));
        assert_eq!(client.next().await["timeout"], 0.0);
        let mut bounded = pin!(ui.input("t".into(), None, timed(Some(1000.0))));
        assert_eq!(client.next().await["timeout"], 1000.0);
        assert!(poll_once(bounded.as_mut()).is_pending());
        tokio::time::advance(Duration::from_millis(999)).await;
        assert!(poll_once(bounded.as_mut()).is_pending());
        tokio::time::advance(Duration::from_millis(1)).await;
        assert!(matches!(poll_once(bounded.as_mut()), Poll::Ready(Ok(None))));
        tokio::time::advance(Duration::from_secs(3600)).await;
        assert!(poll_once(waiting.as_mut()).is_pending());
        assert!(poll_once(zero.as_mut()).is_pending());
    });
}

/// A response that arrives before an abort settles the dialog, however late the dialog is
/// first awaited.
#[test]
fn dialog_response_before_abort_wins_when_awaited_late() {
    on_task_set(|_| async {
        let (_h, _runtime, mut client, ui) = bound("response-first").await;
        // Both settlements are ready when each dialog is first awaited.
        for _ in 0..16 {
            let signal = Cancellation::new();
            let options = ExtensionUIDialogOptions {
                signal: Some(signal.clone()),
                timeout: None,
            };
            let select = ui.select("t".into(), vec!["a".into()], Some(options));
            let request = client.next().await;
            respond(&client, &request, &json!({"value": "a"}));
            nothing_written(&mut client).await;
            signal.abort();
            assert_eq!(select.await.unwrap().as_deref(), Some("a"));
        }
    });
}

/// A response that arrives after the deadline does not count, however late the dialog is
/// first awaited.
#[test]
fn dialog_deadline_before_response_wins_when_awaited_late() {
    on_paused_task_set(|_| async {
        let (_h, _runtime, mut client, ui) = bound("deadline-first").await;
        for _ in 0..16 {
            let options = ExtensionUIDialogOptions {
                signal: None,
                timeout: Some(1000.0),
            };
            let input = ui.input("t".into(), None, Some(options));
            let request = client.next().await;
            tokio::time::advance(Duration::from_millis(1000)).await;
            respond(&client, &request, &json!({"value": "late"}));
            nothing_written(&mut client).await;
            assert_eq!(input.await.unwrap(), None);
        }
    });
}

/// An editor has no timeout or signal: it waits until the client answers.
#[test]
fn editor_has_no_deadline_or_signal() {
    on_paused_task_set(|_| async {
        let (_h, _runtime, mut client, ui) = bound("editor").await;
        let mut editor = pin!(ui.editor("t".into(), None));
        let request = client.next().await;
        assert_eq!(request.get("timeout"), None);
        tokio::time::advance(Duration::from_secs(3600)).await;
        assert!(poll_once(editor.as_mut()).is_pending());
        respond(&client, &request, &json!({"value": "done"}));
        assert_eq!(editor.await.unwrap().as_deref(), Some("done"));
    });
}

/// Notices keep their members and leave out absent ones, empty strings included; a paste
/// sets the editor text.
#[test]
fn notifications_and_editor_commands_keep_omissions() {
    on_task_set(|_| async {
        let (_h, _runtime, mut client, ui) = bound("notices").await;
        ui.notify(String::new(), None).unwrap();
        ui.notify("m".into(), Some(NotificationType::Warning))
            .unwrap();
        ui.notify("m".into(), Some(NotificationType::Error))
            .unwrap();
        ui.set_status("key".into(), Some(String::new())).unwrap();
        ui.set_status("key".into(), None).unwrap();
        ui.set_title(String::new()).unwrap();
        ui.set_editor_text(String::new()).unwrap();
        ui.paste_to_editor("paste".into()).unwrap();
        let mut seen = HashSet::new();
        for expected in [
            json!({"method": "notify", "message": ""}),
            json!({"method": "notify", "message": "m", "notifyType": "warning"}),
            json!({"method": "notify", "message": "m", "notifyType": "error"}),
            json!({"method": "setStatus", "statusKey": "key", "statusText": ""}),
            json!({"method": "setStatus", "statusKey": "key"}),
            json!({"method": "setTitle", "title": ""}),
            json!({"method": "set_editor_text", "text": ""}),
            json!({"method": "set_editor_text", "text": "paste"}),
        ] {
            let mut expected = expected;
            expected["type"] = json!("extension_ui_request");
            assert_eq!(without_fresh_id(client.next().await, &mut seen), expected);
        }
    });
}

/// Widget lines keep their placement or its absence and clearing writes only the key; a
/// component factory needs a terminal, so it is never built and writes nothing.
#[test]
fn widgets_keep_lines_and_ignore_factories() {
    on_task_set(|_| async {
        let (_h, _runtime, mut client, ui) = bound("widgets").await;
        let lines = |lines: &[&str]| {
            Some(WidgetContent::Lines(
                lines.iter().map(|line| (*line).to_owned()).collect(),
            ))
        };
        let placed = |placement| Some(ExtensionWidgetOptions { placement });
        let factory: ComponentFactory =
            facade().ui_callback(|_, _| panic!("a widget factory was built"));
        ui.set_widget("w".into(), Some(WidgetContent::Factory(factory)), None)
            .unwrap();
        ui.set_widget(
            "w".into(),
            lines(&["a", "b"]),
            placed(Some(WidgetPlacement::BelowEditor)),
        )
        .unwrap();
        ui.set_widget(
            "w".into(),
            lines(&[]),
            placed(Some(WidgetPlacement::AboveEditor)),
        )
        .unwrap();
        ui.set_widget("w".into(), lines(&[]), placed(None)).unwrap();
        ui.set_widget("w".into(), None, None).unwrap();
        let mut seen = HashSet::new();
        for expected in [
            json!({"widgetLines": ["a", "b"], "widgetPlacement": "belowEditor"}),
            json!({"widgetLines": [], "widgetPlacement": "aboveEditor"}),
            json!({"widgetLines": []}),
            json!({}),
        ] {
            let mut expected = expected;
            expected["type"] = json!("extension_ui_request");
            expected["method"] = json!("setWidget");
            expected["widgetKey"] = json!("w");
            assert_eq!(without_fresh_id(client.next().await, &mut seen), expected);
        }
        nothing_written(&mut client).await;
    });
}

/// Each terminal-only operation keeps its headless default and writes nothing.
#[test]
fn unsupported_ui_methods_keep_each_default() {
    on_task_set(|_| async {
        let (_h, _runtime, mut client, ui) = bound("defaults").await;
        let api = facade();
        let unsubscribe = ui
            .on_terminal_input(api.ui_callback(|_, _| panic!("terminal input was delivered")))
            .unwrap();
        unsubscribe().unwrap();
        ui.set_working_message(Some("working".into())).unwrap();
        ui.set_working_visible(false).unwrap();
        ui.set_working_indicator(Some(WorkingIndicatorOptions::default()))
            .unwrap();
        ui.set_hidden_thinking_label(Some("hidden".into())).unwrap();
        let footer: FooterFactory = api.ui_callback(|_, _| panic!("a footer was built"));
        ui.set_footer(Some(footer)).unwrap();
        let header: ComponentFactory = api.ui_callback(|_, _| panic!("a header was built"));
        ui.set_header(Some(header)).unwrap();
        let custom: CustomComponentFactory =
            api.ui_callback(|_, _| panic!("a custom component was built"));
        ui.custom_component(custom, None).await.unwrap();
        let autocomplete: AutocompleteProviderFactory =
            api.ui_callback(|_, _| panic!("an autocomplete provider was built"));
        ui.add_autocomplete_provider(autocomplete).unwrap();
        let editor: EditorFactory = api.ui_callback(|_, _| panic!("an editor was built"));
        ui.set_editor_component(Some(editor)).unwrap();
        assert!(ui.get_editor_component().unwrap().is_none());
        assert_eq!(ui.get_editor_text().unwrap(), "");
        assert!(ui.get_all_themes().unwrap().is_empty());
        assert!(ui.get_theme("dark".into()).unwrap().is_none());
        ui.set_tools_expanded(true).unwrap();
        assert!(!ui.get_tools_expanded().unwrap());
        nothing_written(&mut client).await;
    });
}

/// The interface reads the live theme as it is replaced, and selecting a theme is refused
/// without changing it.
#[test]
fn ui_theme_follows_live_replacement() {
    on_task_set(|_| async {
        let (_h, runtime, _) = runtime_for("theme", Vec::new()).await;
        let runtime = Rc::new(runtime);
        let state =
            ThemeState::new(maestro_app::configuration::config::get_theme_directories().unwrap());
        let mut client = serve(Rc::clone(&runtime), no_workspace(), state.theme());
        client.ask(r#"{"type":"get_state"}"#).await;
        let ui = runtime.session().extension_runner().get_ui_context();
        assert!(ui.theme().get().is_err(), "nothing published yet");
        let theme = || {
            Rc::new(
                Theme::new(
                    [],
                    [],
                    maestro_theme::ColorMode::Color256,
                    maestro_theme::ThemeOptions::default(),
                )
                .unwrap(),
            )
        };
        let first = theme();
        state.set_theme_instance(Rc::clone(&first)).unwrap();
        assert!(Rc::ptr_eq(&ui.theme().get().unwrap(), &first));
        let second = theme();
        state.set_theme_instance(Rc::clone(&second)).unwrap();
        assert!(Rc::ptr_eq(&ui.theme().get().unwrap(), &second));
        for selection in [
            ThemeSelection::Name("light".into()),
            ThemeSelection::Theme(published()),
        ] {
            let result = ui.set_theme(selection).unwrap();
            assert!(!result.success);
            assert_eq!(
                result.error.as_deref(),
                Some("Theme switching not supported in RPC mode")
            );
        }
        assert!(Rc::ptr_eq(&ui.theme().get().unwrap(), &second));
    });
}

/// A present member of the wrong kind, `null` included, fails the dialog it answers and
/// writes nothing, during the initial binding and after it; a dismissal still wins.
#[test]
fn malformed_dialog_values_fail_without_wire_reply() {
    on_task_set(|_| async {
        let outcomes: Outcomes = Rc::default();
        let factory = startup_select(Rc::clone(&outcomes));
        let (_h, runtime, _) = runtime_for("malformed", vec![(None, factory)]).await;
        let runtime = Rc::new(runtime);
        let mut client = connect(Rc::clone(&runtime));
        let request = client.next().await;
        assert_eq!(request["title"], "start");
        respond(&client, &request, &json!({"value": null}));
        let state = client.ask(r#"{"id":"bound","type":"get_state"}"#).await;
        assert_eq!(state["id"], "bound", "nothing came before the first answer");
        assert_eq!(
            *outcomes.borrow(),
            [Err("invalid type: null, expected a string".to_owned())]
        );
        let ui = runtime.session().extension_runner().get_ui_context();
        for (answer, error) in [
            (
                json!({"value": 5}),
                "invalid type: integer `5`, expected a string",
            ),
            (
                json!({"value": "a", "cancelled": "x"}),
                "invalid type: string \"x\", expected a boolean",
            ),
            (
                json!({"confirmed": null}),
                "invalid type: null, expected a boolean",
            ),
        ] {
            let select = ui.select("t".into(), vec!["a".into()], None);
            let request = client.next().await;
            respond(&client, &request, &answer);
            assert_eq!(select.await.unwrap_err().to_string(), error, "{answer}");
            nothing_written(&mut client).await;
        }
        let confirm = ui.confirm("t".into(), "m".into(), None);
        let request = client.next().await;
        respond(&client, &request, &json!({"confirmed": "yes"}));
        assert_eq!(
            confirm.await.unwrap_err().to_string(),
            "invalid type: string \"yes\", expected a boolean"
        );
        let select = ui.select("t".into(), vec!["a".into()], None);
        let request = client.next().await;
        respond(&client, &request, &json!({"cancelled": true, "value": 5}));
        assert_eq!(
            select.await.unwrap(),
            None,
            "a dismissal wins over a malformed value"
        );
    });
}
