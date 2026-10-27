//! Real registration and action owners behind the local API endpoint.
use crate::read_imports::ReadEndpoint;
use crate::{api_reads::Request, local_resources::Pump};
use maestro_extensions::{ExtensionAPI, ExtensionActions, Local, create_extension_runtime};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// Creates an API while keeping its weakly referenced registration owner alive.
pub(crate) fn api(
    runtime: Rc<maestro_extensions::ExtensionRuntime<Local>>,
) -> (
    maestro_extensions::types::ExtensionHandle<Local>,
    ExtensionAPI<Local>,
) {
    let source = serde_json::from_value(serde_json::json!({"path":"/api", "source":"cli", "scope":"temporary", "origin":"top-level"})).unwrap();
    let owner = Rc::new(RwLock::new(maestro_extensions::Extension::new(
        "/api".into(),
        "/api".into(),
        Arc::new(RwLock::new(source)),
    )));
    let events = maestro_extensions::create_event_bus_for::<Local>(Rc::new(drop), Rc::new(|_| {}))
        .for_extension();
    let api = ExtensionAPI::new(&owner, runtime, events, "/factory-only".into());
    (owner, api)
}

#[test]
fn flags_keep_native_registration_and_independently_typed_defaults() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            use maestro_extensions::types::{ExtensionFlagOptions, FlagType, FlagValue};
            let runtime = create_extension_runtime::<Local>();
            let (owner, api) = api(runtime.clone());
            let (pump, admission) = Pump::new(crate::tests::support::widgets());
            let endpoint = pump.registry().api(api).unwrap();
            let worker = std::thread::spawn(move || {
                assert_eq!(
                    endpoint
                        .read(|reply| Request::Flag(" mode ".into(), reply))
                        .unwrap(),
                    None
                );
                for default in [
                    Some(FlagValue::String(" original ".into())),
                    Some(FlagValue::Boolean(false)),
                ] {
                    let options = ExtensionFlagOptions {
                        description: Some(String::new()),
                        r#type: FlagType::Boolean,
                        default,
                    };
                    endpoint
                        .read(|reply| Request::RegisterFlag(" mode ".into(), options, reply))
                        .unwrap();
                }
                assert_eq!(
                    endpoint
                        .read(|reply| Request::Flag(" mode ".into(), reply))
                        .unwrap(),
                    Some(FlagValue::String(" original ".into()))
                );
            });
            drop(admission);
            pump.serve().await;
            worker.join().unwrap();
            let owner = owner.read().unwrap();
            let flag = owner.flags[" mode "].read().unwrap();
            assert_eq!(flag.r#type, FlagType::Boolean);
            assert_eq!(flag.default, Some(FlagValue::Boolean(false)));
            assert_eq!(flag.description.as_deref(), Some(""));
            assert_eq!(flag.extension_path, "/api");
        });
}

#[test]
fn actions_select_live_runtime_when_consumed_and_keep_native_errors() {
    tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(async {
        let runtime = create_extension_runtime::<Local>();
        let (_owner, api) = api(runtime.clone());
        let (mut pump, admission) = Pump::new(crate::tests::support::widgets());
        let endpoint = pump.registry().api(api).unwrap();
        let reader = endpoint.clone();
        let worker = std::thread::spawn(move || reader.read(Request::ActiveTools));
        assert!(pump.answer_next().await);
        assert_eq!(worker.join().unwrap().unwrap_err(), "Extension runtime not initialized. Action methods cannot be called during extension loading.");
        runtime.actions.write().unwrap().get_active_tools = Rc::new(|| Ok(vec![" real tool ".into(), String::new()]));
        let reader = endpoint.clone();
        let worker = std::thread::spawn(move || reader.read(Request::ActiveTools));
        assert!(pump.answer_next().await);
        assert_eq!(worker.join().unwrap().unwrap(), [" real tool ", ""]);
        runtime.invalidate(Some(String::new()));
        let worker = std::thread::spawn(move || endpoint.read(Request::SessionName));
        drop(admission);
        pump.serve().await;
        assert_eq!(worker.join().unwrap().unwrap_err(), "");
    });
}

#[test]
fn message_and_entry_actions_preserve_presence_and_original_operands() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(messages());
}

/// Records actual runtime operands without replacing the API's action selection.
async fn messages() {
    let runtime = create_extension_runtime::<Local>();
    let (_owner, api) = api(runtime.clone());
    let seen = Rc::new(RefCell::new(Vec::new()));
    let entries = seen.clone();
    let entry_values = Rc::new(RefCell::new(Vec::new()));
    let received_entries = entry_values.clone();
    let custom = seen.clone();
    let users = seen.clone();
    let original = runtime.actions.read().unwrap().clone();
    *runtime.actions.write().unwrap() = ExtensionActions {
        append_entry: Rc::new(move |name, data| {
            received_entries.borrow_mut().push(data.clone());
            entries.borrow_mut().push(serde_json::json!([name, data]));
            Ok(())
        }),
        send_message: Rc::new(move |message, options| {
            custom
                .borrow_mut()
                .push(serde_json::json!([message, options]));
            Ok(())
        }),
        send_user_message: Rc::new(move |content, options| {
            users
                .borrow_mut()
                .push(serde_json::json!([content, options]));
            Ok(())
        }),
        ..original
    };
    let (pump, admission) = Pump::new(crate::tests::support::widgets());
    let endpoint = pump.registry().api(api).unwrap();
    let worker = std::thread::spawn(move || send_messages(&endpoint));
    drop(admission);
    pump.serve().await;
    worker.join().unwrap();
    check_messages(&seen.borrow(), &entry_values.borrow());
}

/// Checks original presence separately from the serialized observation document.
fn check_messages(seen: &[serde_json::Value], entry_values: &[Option<serde_json::Value>]) {
    assert_eq!(
        entry_values,
        &[
            None,
            Some(serde_json::Value::Null),
            Some(serde_json::json!({"x":false}))
        ]
    );
    assert_eq!(seen.len(), 8);
    assert_eq!(seen[0], serde_json::json!([" raw\u{feff}\u{85} ", null]));
    assert_eq!(
        seen[2],
        serde_json::json!([" raw\u{feff}\u{85} ", {"x":false}])
    );
    assert!(seen[3][0].get("details").is_none());
    assert_eq!(seen[4][0]["details"], serde_json::Value::Null);
    assert_eq!(seen[5][0]["details"], serde_json::json!({"x":false}));
    assert_eq!(seen[3][1], serde_json::Value::Null);
    assert_eq!(seen[4][1], serde_json::json!({}));
    assert_eq!(
        seen[5][1],
        serde_json::json!({"triggerTurn":true,"deliverAs":"nextTurn"})
    );
    assert_eq!(seen[6], serde_json::json!([" raw ", null]));
    assert_eq!(seen[7][0][1]["mimeType"], "image/png");
    assert_eq!(seen[7][1], serde_json::json!({"deliverAs":"followUp"}));
}

/// Supplies absent, empty and full records through the Store-safe endpoint.
fn send_messages(endpoint: &crate::api_reads::Endpoint) {
    for data in [None, Some("null"), Some(r#"{"x":false}"#)] {
        endpoint
            .read(|reply| {
                Request::Append(" raw\u{feff}\u{85} ".into(), data.map(str::to_owned), reply)
            })
            .unwrap();
    }
    for (message, options) in [
        (
            r#"{"customType":" raw ","content":"","display":false}"#,
            None,
        ),
        (
            r#"{"customType":" raw ","content":"","display":false,"details":null}"#,
            Some("{}"),
        ),
        (
            r#"{"customType":" raw ","content":"","display":false,"details":{"x":false}}"#,
            Some(r#"{"triggerTurn":true,"deliverAs":"nextTurn"}"#),
        ),
    ] {
        endpoint
            .read(|reply| Request::SendMessage(message.into(), options.map(str::to_owned), reply))
            .unwrap();
    }
    endpoint
        .read(|reply| Request::SendUserMessage(r#"" raw ""#.into(), None, reply))
        .unwrap();
    endpoint.read(|reply| Request::SendUserMessage(r#"[{"type":"text","text":""},{"type":"image","data":"AA==","mimeType":"image/png"}]"#.into(), Some(r#"{"deliverAs":"followUp"}"#.into()), reply)).unwrap();
    assert!(
        endpoint
            .read(|reply| Request::SendMessage("not json".into(), None, reply))
            .is_err()
    );
}

mod actions;

/// Construction retains the application's owners and rejects an absent or stale facade.
#[test]
fn tool_construction_retains_application_services() {
    let runtime = create_extension_runtime::<Local>();
    let (_owner, api) = api(runtime.clone());
    assert_eq!(
        api.tool_construction().err().unwrap(),
        "Extension tool construction is not configured"
    );
    let manager = Arc::new(maestro_extensions::ToolsManager::new(
        "/app/bin".into(),
        None,
    ));
    runtime.set_tool_construction(maestro_extensions::ToolConstruction {
        tools_manager: manager.clone(),
        bin_dir: "/app/bin".into(),
    });
    let services = api.tool_construction().unwrap();
    assert!(Arc::ptr_eq(&services.tools_manager, &manager));
    assert_eq!(services.bin_dir, "/app/bin");
    runtime.invalidate(Some("ended tools".into()));
    assert_eq!(api.tool_construction().err().unwrap(), "ended tools");
}
