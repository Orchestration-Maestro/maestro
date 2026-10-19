//! Failed authored initialization releases the Store and its original native request owner.
#![cfg(test)]
use maestro_extensions::{
    ConfigValueOperations, ExtensionRunner, Local, create_event_bus_for, create_extension_runtime,
    load_extension_from_factory, types::FlagValue,
};
use maestro_extensions_wasmtime::artifact_factory;
use std::{rc::Rc, sync::RwLock};

/// The pump alone retains these supplied effects after factory initialization returns.
struct Released(Option<tokio::sync::oneshot::Sender<()>>);
impl ConfigValueOperations for Released {
    fn environment(&self, _: &str) -> Option<String> {
        panic!("no credential read");
    }
    fn execute(&self, _: &str) -> Option<Vec<u8>> {
        panic!("no credential execution");
    }
}
impl Drop for Released {
    fn drop(&mut self) {
        if let Some(released) = self.0.take() {
            let _ = released.send(());
        }
    }
}

#[test]
fn failed_authored_startup_releases_the_native_pump_after_store_teardown() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
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
        let (released, teardown) = tokio::sync::oneshot::channel();
        let path = std::env::var("MAESTRO_CONTEXT_COMPONENT").unwrap();
        let error = load_extension_from_factory(
            artifact_factory(path.clone(), Rc::new(Released(Some(released)))),
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
        teardown.await.unwrap();
        assert!(runner.get_command("bridge-observe").is_none());
    });
}
