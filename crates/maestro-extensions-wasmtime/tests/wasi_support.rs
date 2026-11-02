//! Real artifact instances and owned observations shared by standard-interface tests.
#![cfg(test)]
use super::components::TracedBus;
use maestro_extensions::{
    ExtensionFactory, ExtensionRunner, Local, ResourcesDiscoverReason, create_extension_runtime,
    load_extension_from_factory,
};
use maestro_extensions_wasmtime::artifact_factory;
use serde_json::Value;
use std::{cell::RefCell, path::Path, rc::Rc, sync::RwLock};

/// Setup's real compiled component.
pub const COMPONENT: &str = "MAESTRO_WASI_COMPONENT";

/// Runs on the caller's local executor with actual native I/O and timer drivers.
pub use super::example_process::run;

/// One independently live component and the existing notification route it reports through.
pub struct Harness {
    /// The original native runner and its retained registration owners.
    pub runner: ExtensionRunner<Local>,
    /// Only explicitly authored observations are collected.
    pub bus: TracedBus,
    /// Existing attributed callback errors, without a second diagnostic channel.
    errors: Rc<RefCell<Vec<String>>>,
}
impl Harness {
    /// Loads the actual artifact without substituting a host or a recorder for standard I/O.
    pub async fn load(path: &Path) -> Self {
        Self::factory(artifact_factory(
            path.to_str().unwrap().into(),
            super::support::widgets(),
            maestro_extensions_wasmtime::Installation {
                version: "installed-component-host".into(),
                agent_dir: "/controlled-agent".into(),
            },
        ))
        .await
    }

    /// Uses the same real host with a caller-selected standard output route.
    pub async fn factory(factory: ExtensionFactory<Local>) -> Self {
        let bus = TracedBus::new();
        let runtime = create_extension_runtime::<Local>();
        let owner = load_extension_from_factory(
            factory,
            "/work".into(),
            bus.controller.for_extension(),
            runtime.clone(),
            None,
        )
        .await
        .unwrap();
        let runner = ExtensionRunner::<Local>::new(
            Rc::new(RwLock::new(vec![owner])),
            runtime,
            super::session::runner_session("/work").0,
            Rc::new(|_| {}),
            maestro_theme::LiveTheme::default(),
        );
        super::support::bind_prompt(&runner, "wasi context");
        let errors = Rc::new(RefCell::new(Vec::new()));
        let received = errors.clone();
        let _keep = runner.on_error(Rc::new(move |error| {
            received.borrow_mut().push(error.error.clone());
            Ok(())
        }));
        Self {
            runner,
            bus,
            errors,
        }
    }

    /// Invokes an ordinary resource callback on this instance, with literal fixture operands.
    pub async fn request(&self, request: Value) -> Value {
        self.outcome(request).await.unwrap()
    }

    /// Captures only the existing callback/fault diagnostic when an operation fails.
    pub async fn outcome(&self, request: Value) -> Result<Value, Vec<String>> {
        self.runner
            .emit_resources_discover(request.to_string(), ResourcesDiscoverReason::Startup)
            .await
            .unwrap();
        let errors = self.errors.take();
        if !errors.is_empty() {
            return Err(errors);
        }
        let observations = self.bus.take();
        assert_eq!(observations.len(), 1, "{observations:?}");
        Ok(observations.into_iter().next().unwrap())
    }
}
