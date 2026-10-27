//! Canonical retained components use the production factory and native renderer owner.
#![cfg(test)]
use maestro_extensions::types::ExtensionHandle;
use maestro_extensions::{
    ComponentHandle, EventBus, EventError, ExtensionRunner, Local, MessageRenderOptions,
    create_event_bus_for, create_extension_runtime, load_extension_from_factory,
};
use std::{rc::Rc, sync::RwLock};

use super::{session, support};

/// Loads the same author component through the one production factory.
async fn loaded() -> (
    ExtensionHandle<Local>,
    ExtensionRunner<Local>,
    EventBus<Local>,
) {
    loaded_with(|_| {}).await
}

/// Installs an observation before guest registrations to prove mixed listener order.
async fn loaded_with(
    before: impl FnOnce(&EventBus<Local>),
) -> (
    ExtensionHandle<Local>,
    ExtensionRunner<Local>,
    EventBus<Local>,
) {
    let bus = create_event_bus_for::<Local>(
        Rc::new(|_| panic!("no tails")),
        Rc::new(|_| panic!("no errors")),
    );
    let events = bus.for_extension();
    before(&events);
    loaded_on(events).await
}

/// Loads the canonical fixture using a bus with caller-controlled diagnostics and tails.
pub(super) async fn loaded_on(
    events: EventBus<Local>,
) -> (
    ExtensionHandle<Local>,
    ExtensionRunner<Local>,
    EventBus<Local>,
) {
    loaded_in(events, session::runner_session("/render-domain").0).await
}

/// Keeps the original loader runtime bound to the caller's actual session owners.
pub(super) async fn loaded_in(
    events: EventBus<Local>,
    session: maestro_extensions::runner::ExtensionRunnerSession,
) -> (
    ExtensionHandle<Local>,
    ExtensionRunner<Local>,
    EventBus<Local>,
) {
    loaded_at(events, session, "/fixture").await
}

/// Supplies the factory's captured directory independently of the callback context.
pub(super) async fn loaded_at(
    events: EventBus<Local>,
    session: maestro_extensions::runner::ExtensionRunnerSession,
    cwd: &str,
) -> (
    ExtensionHandle<Local>,
    ExtensionRunner<Local>,
    EventBus<Local>,
) {
    let runtime = create_extension_runtime::<Local>();
    runtime.set_catalog_construction(maestro_extensions::CatalogConstruction {
        auth_path: std::sync::Arc::new(|| Err("unexpected default auth path".into())),
        models_path: std::sync::Arc::new(|| Err("unexpected default model path".into())),
        config_values: session.model_registry.config_values().clone(),
    });
    loaded_with_runtime(events, session, cwd, runtime).await
}

/// Loads the real artifact after the caller installed factory-time defaults and flags.
pub(super) async fn loaded_with_runtime(
    events: EventBus<Local>,
    session: maestro_extensions::runner::ExtensionRunnerSession,
    cwd: &str,
    runtime: Rc<maestro_extensions::ExtensionRuntime<Local>>,
) -> (
    ExtensionHandle<Local>,
    ExtensionRunner<Local>,
    EventBus<Local>,
) {
    let result = unbound_with_runtime(events, session, cwd, runtime).await;
    support::bind_prompt(&result.1, "renderer barrier");
    result
}

/// Loads factories without binding so callers can observe individual flush diagnostics.
pub(super) async fn unbound_with_runtime(
    events: EventBus<Local>,
    session: maestro_extensions::runner::ExtensionRunnerSession,
    cwd: &str,
    runtime: Rc<maestro_extensions::ExtensionRuntime<Local>>,
) -> (
    ExtensionHandle<Local>,
    ExtensionRunner<Local>,
    EventBus<Local>,
) {
    let path = std::env::var("MAESTRO_CONTEXT_COMPONENT").unwrap();
    let owner = load_extension_from_factory(
        crate::artifact(path.clone()),
        cwd.into(),
        events.clone(),
        runtime.clone(),
        Some(path),
    )
    .await
    .unwrap();
    let runner = ExtensionRunner::<Local>::new(
        Rc::new(RwLock::new(vec![owner.clone()])),
        runtime,
        session,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    );
    (owner, runner, events)
}

/// Invokes the native Local renderer with a real callback frame and complete message.
pub(super) fn view(
    runner: &ExtensionRunner<Local>,
    bus: &EventBus<Local>,
    name: &str,
) -> Result<Option<ComponentHandle>, EventError> {
    let renderer = runner
        .get_message_renderer::<serde_json::Value>("bridge-view")
        .unwrap();
    let message = serde_json::from_value(serde_json::json!({"role":"custom", "customType": name, "content": "literal content", "display": true, "timestamp": 12.5})).unwrap();
    bus.invoke_callback(|emitter| {
        renderer(
            Rc::new(message),
            MessageRenderOptions { expanded: true },
            maestro_theme::LiveTheme::default(),
            emitter,
        )
    })
}

#[test]
fn canonical_renderer_preserves_creation_render_and_infallible_method_errors() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let (_owner, runner, bus) = loaded().await;
        assert_eq!(
            view(&runner, &bus, "creation-error")
                .err()
                .unwrap()
                .to_string(),
            "guest creation failure"
        );
        let failing = view(&runner, &bus, "render-error").unwrap().unwrap();
        assert_eq!(
            failing.try_render(80).unwrap_err().to_string(),
            "guest render failure"
        );
        let normal = view(&runner, &bus, "normal").unwrap().unwrap();
        assert_eq!(normal.try_render(37).unwrap(), ["normal:37"]);
        normal.input_handler().unwrap().handle_input("error");
        assert_eq!(
            normal.try_render(38).unwrap_err().to_string(),
            "guest input failure"
        );
        assert_eq!(normal.try_render(39).unwrap(), ["normal:39"]);
        assert!(normal.wants_key_release());
        let key = view(&runner, &bus, "key-error").unwrap().unwrap();
        assert!(!key.wants_key_release());
        assert_eq!(
            key.try_render(1).unwrap_err().to_string(),
            "guest key failure"
        );
        assert_eq!(key.try_render(2).unwrap(), ["key-error:2"]);
        let invalidation = view(&runner, &bus, "invalidate-error").unwrap().unwrap();
        invalidation.invalidate();
        assert_eq!(
            invalidation.try_render(1).unwrap_err().to_string(),
            "guest invalidate failure"
        );
        assert_eq!(invalidation.try_render(2).unwrap(), ["invalidate-error:2"]);
    });
}

#[test]
fn final_component_alias_drops_once_before_the_next_store_call() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let (owner, runner, bus) = loaded().await;
        let component = view(&runner, &bus, "final-alias").unwrap().unwrap();
        let alias = component.clone();
        drop(component);
        let command = runner
            .get_command("bridge-observe")
            .unwrap()
            .command
            .handler;
        assert_eq!(
            command("barrier".into(), runner.create_command_context())
                .await
                .unwrap_err()
                .to_string(),
            "command:barrier|/render-domain|renderer barrier"
        );
        assert!(
            !owner
                .read()
                .unwrap()
                .flags
                .contains_key("dropped-final-alias")
        );
        drop(alias);
        assert_eq!(
            command("barrier".into(), runner.create_command_context())
                .await
                .unwrap_err()
                .to_string(),
            "command:barrier|/render-domain|renderer barrier"
        );
        assert!(
            owner
                .read()
                .unwrap()
                .flags
                .contains_key("dropped-final-alias")
        );
    });
}

#[test]
fn same_component_reentry_preserves_mixed_native_and_guest_registration_order() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let trace = Rc::new(std::cell::RefCell::new(Vec::new()));
        let before = trace.clone();
        let (_owner, runner, bus) = loaded_with(move |events| {
            let observed = before.clone();
            events.on(
                "trace",
                Rc::new(move |data, _| {
                    observed
                        .borrow_mut()
                        .push(data.as_str().unwrap().to_owned());
                    Ok(None)
                }),
            );
            events.on(
                "renderer-created",
                Rc::new(move |data, _| {
                    before
                        .borrow_mut()
                        .push(format!("native-before:{}", data[0].as_str().unwrap()));
                    Ok(None)
                }),
            );
        })
        .await;
        let after = trace.clone();
        bus.on(
            "renderer-created",
            Rc::new(move |data, _| {
                after
                    .borrow_mut()
                    .push(format!("native-after:{}", data[0].as_str().unwrap()));
                Ok(None)
            }),
        );
        let component = view(&runner, &bus, "reentry").unwrap().unwrap();
        assert_eq!(
            *trace.borrow(),
            [
                "native-before:reentry",
                "g1:reentry",
                "native-before:inner",
                "g1:inner",
                "g2:inner",
                "native-after:inner",
                "g2:reentry",
                "native-after:reentry",
            ]
        );
        assert_eq!(component.try_render(31).unwrap(), ["reentry:31"]);
    });
}

/// Binds independently controlled native waits so a real trap can strand another call.
fn controlled_wait(
    runner: &ExtensionRunner<Local>,
    label: &'static str,
    entered: tokio::sync::mpsc::UnboundedSender<(&'static str, tokio::sync::oneshot::Sender<()>)>,
    entries: Rc<std::cell::Cell<usize>>,
) {
    use maestro_extensions::{
        ExtensionCommandContextActions, ExtensionContextActions, ExtensionExecution,
        types::ProviderActions,
    };
    runner
        .bind_core(
            Local::unbound_actions(),
            ExtensionContextActions {
                get_system_prompt: Rc::new(move || {
                    entries.set(entries.get() + 1);
                    Ok(label.into())
                }),
                ..Default::default()
            },
            ProviderActions::default(),
        )
        .unwrap();
    runner.bind_command_context(Some(ExtensionCommandContextActions::new(
        maestro_extensions::CommandContextOperations {
            wait_for_idle: Rc::new(move || {
                let (resume, result) = tokio::sync::oneshot::channel();
                entered.send((label, resume)).unwrap();
                Box::pin(async move {
                    result.await?;
                    Ok(())
                })
            }),
            ..crate::context_actions::silent_command_operations()
        },
    )));
}

#[test]
fn overlapping_guest_trap_settles_all_calls_and_skips_queued_destructors_and_later_entry() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, faulted_calls(true));
}

#[test]
fn canonical_trap_settles_overlapping_calls_and_caches_the_fault_without_renderer_entry() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, faulted_calls(false));
}

/// Exercises the same fault owner with and without a final guest-resource release queued.
async fn faulted_calls(queue_release: bool) {
    let (owner, trap_runner, bus) = loaded().await;
    let pending_runner = other_runner(&owner);
    let (entered, mut incoming) = tokio::sync::mpsc::unbounded_channel();
    let entries = Rc::new(std::cell::Cell::new(0));
    controlled_wait(&trap_runner, "trap", entered.clone(), entries.clone());
    controlled_wait(&pending_runner, "pending", entered, entries.clone());
    let component = queue_release.then(|| {
        view(&trap_runner, &bus, "queued-on-fault")
            .unwrap()
            .unwrap()
    });
    let command = trap_runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    let (first, second, pending) = futures_util::future::join3(
        command("trap".into(), trap_runner.create_command_context()),
        command("pending".into(), pending_runner.create_command_context()),
        async {
            let a = incoming.recv().await.unwrap();
            let b = incoming.recv().await.unwrap();
            let (trap, pending) = if a.0 == "trap" {
                (a.1, b.1)
            } else {
                (b.1, a.1)
            };
            drop(component);
            trap.send(()).unwrap();
            pending
        },
    )
    .await;
    let first = first.unwrap_err().to_string();
    assert!(
        first.contains("component fault") && first.contains("wasm trap"),
        "{first}"
    );
    assert_eq!(second.unwrap_err().to_string(), first);
    assert_cached_fault(&trap_runner, &first, &entries).await;
    assert!(
        !owner
            .read()
            .unwrap()
            .flags
            .contains_key("dropped-queued-on-fault")
    );
    assert!(
        owner
            .read()
            .unwrap()
            .commands
            .contains_key("bridge-observe")
    );
    assert!(
        pending.send(()).is_ok(),
        "native operation was automatically cancelled"
    );
}

/// Creates an independent caller for a command that remains pending during another caller's trap.
fn other_runner(owner: &ExtensionHandle<Local>) -> ExtensionRunner<Local> {
    ExtensionRunner::<Local>::new(
        Rc::new(RwLock::new(vec![owner.clone()])),
        create_extension_runtime::<Local>(),
        session::runner_session("/pending").0,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    )
}

/// A cached fault rejects the original registration without another guest-side prompt read.
async fn assert_cached_fault(
    runner: &ExtensionRunner<Local>,
    fault: &str,
    entries: &std::cell::Cell<usize>,
) {
    assert_eq!(entries.get(), 2);
    let command = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    assert_eq!(
        command("later".into(), runner.create_command_context())
            .await
            .unwrap_err()
            .to_string(),
        fault
    );
    assert_eq!(entries.get(), 2, "faulted Store entered the guest again");
}

#[test]
fn foreign_component_listener_prefixes_run_after_the_current_renderer_returns() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let controller = create_event_bus_for::<Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no errors")),
        );
        let bus = controller.for_extension();
        bus.on(
            "trace",
            Rc::new(|data, _| {
                CROSS_TRACE.with_borrow_mut(|trace| trace.push(data.as_str().unwrap().to_owned()));
                Ok(None)
            }),
        );
        let path = std::env::var("MAESTRO_CONTEXT_COMPONENT").unwrap();
        let owner = load_extension_from_factory(
            crate::artifact(path.clone()),
            "/fixture".into(),
            bus.clone(),
            create_extension_runtime::<Local>(),
            Some(path.clone()),
        )
        .await
        .unwrap();
        let runner = other_runner(&owner);
        let _foreign = load_extension_from_factory(
            crate::artifact(path.clone()),
            "/foreign".into(),
            controller.for_extension(),
            create_extension_runtime::<Local>(),
            Some(path),
        )
        .await
        .unwrap();
        bus.on(
            "renderer-created",
            Rc::new(|data, _| {
                CROSS_TRACE.with_borrow_mut(|trace| {
                    trace.push(format!("native-after:{}", data[0].as_str().unwrap()));
                });
                Ok(None)
            }),
        );
        let _component = view(&runner, &bus, "cross-instance").unwrap().unwrap();
        CROSS_TRACE.with_borrow(|trace| {
            assert_eq!(
                trace,
                &[
                    "g1:cross-instance",
                    "g2:cross-instance",
                    "native-after:cross-instance",
                    "renderer-return:cross-instance",
                    "g1:cross-instance",
                    "g2:cross-instance",
                ]
            );
        });
    });
}
thread_local! {
    static CROSS_TRACE: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}
