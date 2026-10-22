//! Extension loading through the resource loader: precedence, conflicts, inline
//! factories, provenance and the event bus extensions share.
#![cfg(unix)]
use crate::resource_fixtures::{
    ExtensionSpec, Root, check_loader_cases, factory, loader, loader_case, on_task_set,
    resource_options, with_isolated_home,
};
use maestro_app::{DefaultEventBus, ExtensionFactory, ResourceLoader};
use maestro_extensions::{EventBus, EventListener, create_event_bus_for};
use maestro_models::{Local, Threaded};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
};
use tokio::{sync::oneshot, task::LocalSet};

#[test]
fn symlinked_extension_directories_load_once() {
    check_loader_cases(
        "symlinked_extension_directories_load_once",
        &["symlinked_extension_directories_load_once"],
    );
}

#[test]
fn colliding_commands_keep_both_extensions() {
    check_loader_cases(
        "colliding_commands_keep_both_extensions",
        &["colliding_commands_keep_both_extensions"],
    );
}

#[test]
fn duplicate_tools_report_conflicts() {
    check_loader_cases(
        "duplicate_tools_report_conflicts",
        &["duplicate_tools_report_conflicts"],
    );
}

#[test]
fn duplicate_flags_report_conflicts() {
    check_loader_cases(
        "duplicate_flags_report_conflicts",
        &["duplicate_flags_report_conflicts"],
    );
}

#[test]
fn explicit_extensions_precede_discovered_ones() {
    check_loader_cases(
        "explicit_extensions_precede_discovered_ones",
        &["explicit_extensions_precede_discovered"],
    );
}

#[test]
fn inline_factories_follow_path_extensions() {
    check_loader_cases(
        "inline_factories_follow_path_extensions",
        &["inline_factories_follow_path_extensions"],
    );
}

#[test]
fn disabled_extensions_keep_explicit_paths() {
    check_loader_cases(
        "disabled_extensions_keep_explicit_paths",
        &["no_extensions_keeps_explicit_paths"],
    );
}

/// Each loaded extension's provenance record is the one its commands and tools share.
#[test]
fn extension_provenance_reaches_commands_and_tools() {
    with_isolated_home("extension_provenance_reaches_commands_and_tools", |base| {
        let case = loader_case("constructor_results_are_empty");
        let root = Root::new(base, "provenance");
        let spec: ExtensionSpec = serde_json::from_value(json!({
            "commands": [["inline-command", "inline"]],
            "tools": [["inline-tool", "inline tool"]],
        }))
        .unwrap();
        let loaded = on_task_set(|local| async move {
            let mut options = resource_options(&root, &case, local);
            options.extension_factories = vec![factory(spec)];
            let loader = loader(&root, &case, options);
            loader.reload().await.unwrap();
            loader.get_extensions()
        });
        let extension = loaded.extensions[0].read().unwrap();
        let shared = &extension.source_info;
        let command = extension.commands["inline-command"].read().unwrap();
        assert!(Arc::ptr_eq(&command.source_info, shared));
        assert!(Arc::ptr_eq(
            &extension.tools["inline-tool"].source_info,
            shared
        ));
        let source = shared.read().unwrap();
        assert_eq!(
            (source.path.as_str(), source.source.as_str()),
            ("<inline:1>", "inline")
        );
    });
}

/// A bus listener of one extension, a continuation flag and the bus another extension emits on.
struct LocalProbe {
    /// The bus captured from the emitting extension.
    events: Rc<RefCell<Option<EventBus<Local>>>>,
    /// Signalled by the listener's continuation.
    finished: Rc<RefCell<Option<oneshot::Sender<()>>>>,
}

/// The listening factory and the capturing factory of a local probe.
fn local_probe_factories(probe: &LocalProbe) -> Vec<ExtensionFactory<Local>> {
    let finished = Rc::clone(&probe.finished);
    let listen: ExtensionFactory<Local> = Rc::new(move |api| {
        let finished = Rc::clone(&finished);
        let listener: EventListener<Local> = Rc::new(move |_, _| {
            let finished = finished.borrow_mut().take();
            Ok(Some(Box::pin(async move {
                tokio::task::yield_now().await;
                finished.map(|done| done.send(()));
                Ok(())
            })))
        });
        drop(api.events.on("probe", listener));
        Box::pin(async { Ok(()) })
    });
    let events = Rc::clone(&probe.events);
    let capture: ExtensionFactory<Local> = Rc::new(move |api| {
        *events.borrow_mut() = Some(api.events);
        Box::pin(async { Ok(()) })
    });
    vec![listen, capture]
}

#[test]
fn default_event_bus_runs_local_tails_on_the_task_set() {
    with_isolated_home(
        "default_event_bus_runs_local_tails_on_the_task_set",
        |base| {
            let case = loader_case("constructor_results_are_empty");
            let root = Root::new(base, "local_bus");
            on_task_set(|local| async move {
                let (sender, receiver) = oneshot::channel();
                let probe = LocalProbe {
                    events: Rc::default(),
                    finished: Rc::new(RefCell::new(Some(sender))),
                };
                let mut options = resource_options(&root, &case, local);
                options.extension_factories = local_probe_factories(&probe);
                let loader = loader(&root, &case, options);
                loader.reload().await.unwrap();
                let events = probe.events.borrow_mut().take().unwrap();
                events.emit("probe", json!({})).await;
                assert!(
                    probe.finished.borrow().is_none(),
                    "the prefix ran during emission"
                );
                receiver.await.unwrap();
            });
        },
    );
}

#[test]
fn default_event_bus_runs_threaded_tails_on_the_task_set() {
    let local = Rc::new(LocalSet::new());
    let bus = Threaded::default_event_bus(&local);
    let (sender, receiver) = oneshot::channel();
    let sender = Arc::new(Mutex::new(Some(sender)));
    drop(bus.on(
        "probe",
        Arc::new(move |_, _| {
            let sender = sender.lock().unwrap().take();
            Ok(Some(Box::pin(async move {
                tokio::task::yield_now().await;
                sender.map(|done| done.send(()));
                Ok(())
            })))
        }),
    ));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    runtime.block_on(local.run_until(async {
        bus.for_extension().emit("probe", json!({})).await;
        receiver.await.unwrap();
    }));
}

/// A failing listener's message is written to standard error as the bus formats it.
#[test]
fn default_event_bus_reports_listener_failures_on_stderr() {
    if maestro_test_process::selection().is_child() {
        let local = Rc::new(LocalSet::new());
        let bus = Local::default_event_bus(&local);
        drop(bus.on("probe", Rc::new(|_, _| Err("listener failed".into()))));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        runtime.block_on(local.run_until(bus.emit("probe", json!({}))));
        return;
    }
    let output = maestro_test_process::TestProcess::current(
        "default_event_bus_reports_listener_failures_on_stderr",
    )
    .unwrap()
    .case("*")
    .nocapture()
    .output()
    .unwrap();
    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("Event handler error (probe): listener failed\n"),
        "{stderr}"
    );
}

/// A factory that emits on `announce` while it loads.
fn announcing() -> ExtensionFactory<Local> {
    Rc::new(|api| {
        Box::pin(async move {
            api.events.emit("announce", json!({"from": "inline"})).await;
            Ok(())
        })
    })
}

#[test]
fn supplied_event_bus_receives_extension_events() {
    with_isolated_home("supplied_event_bus_receives_extension_events", |base| {
        let case = loader_case("constructor_results_are_empty");
        let root = Root::new(base, "supplied_bus");
        let received: Rc<RefCell<Vec<Value>>> = Rc::default();
        let seen = Rc::clone(&received);
        on_task_set(|local| async move {
            let bus = create_event_bus_for::<Local>(Rc::new(drop), Rc::new(|_| {}));
            drop(bus.on(
                "announce",
                Rc::new(move |data, _| {
                    seen.borrow_mut().push((*data).clone());
                    Ok(None)
                }),
            ));
            let mut options = resource_options(&root, &case, local);
            options.event_bus = Some(bus);
            options.extension_factories = vec![announcing()];
            loader(&root, &case, options).reload().await.unwrap();
        });
        assert_eq!(*received.borrow(), [json!({"from": "inline"})]);
    });
}
