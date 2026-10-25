//! Failed authored initialization releases the Store and its original native request owner.
#![cfg(test)]
use maestro_extensions::{
    ExtensionRunner, Local, create_event_bus_for, create_extension_runtime,
    load_extension_from_factory, types::FlagValue,
};
use maestro_extensions_wasmtime::artifact_factory;
use std::{rc::Rc, sync::RwLock};

/// Checks failed startup teardown before starting an independently healthy component.
pub(super) async fn failed() {
    let runtime = create_extension_runtime::<Local>();
    let runner = ExtensionRunner::<Local>::new(
        Rc::new(RwLock::default()),
        runtime.clone(),
        super::session::runner_session("/failed-startup").0,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    );
    runner.set_flag_value("fail-start".into(), FlagValue::Boolean(true));
    let bus = create_event_bus_for::<Local>(
        Rc::new(|_| panic!("no tails")),
        Rc::new(|_| panic!("no reports")),
    );
    let path = std::env::var("MAESTRO_CONTEXT_COMPONENT").unwrap();
    let error = load_extension_from_factory(
        artifact_factory(path.clone()),
        "/failed-startup".into(),
        bus.for_extension(),
        runtime,
        Some(path.clone()),
    )
    .await
    .err()
    .unwrap()
    .to_string();
    assert!(
        error.contains(&path) && error.ends_with("guest startup failure"),
        "{error}"
    );
    assert!(runner.get_command("bridge-observe").is_none());
    let (owner, healthy, events) = super::retained_components::loaded_on(bus.for_extension()).await;
    let command = healthy
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    assert_eq!(
        command("healthy-start".into(), healthy.create_command_context())
            .await
            .unwrap_err()
            .to_string(),
        "command:healthy-start|/render-domain|renderer barrier"
    );
    assert!(owner.read().unwrap().flags.contains_key("fail-start"));
    let component = super::retained_components::view(&healthy, &events, "healthy-start")
        .unwrap()
        .unwrap();
    assert_eq!(component.try_render(47).unwrap(), ["healthy-start:47"]);
    bus.clear();
}
