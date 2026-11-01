//! Built components loaded through the domain loader and driven through their `hooks` command.
#![cfg(test)]
use maestro_extensions::{
    EventBusController, ExtensionRunner, Local, ModuleLoader, Subscription, create_event_bus_for,
    types::ExtensionHandle,
};
use serde_json::Value;
use std::{cell::RefCell, path::Path, rc::Rc};

/// Runs a test body on the local task set the artifact factory requires.
pub fn local(test: impl Future<Output = ()>) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, test);
}

/// Copies the component the setup variable `component` names to `artifact`.
pub fn place(component: &str, artifact: &Path) {
    std::fs::create_dir_all(artifact.parent().unwrap()).unwrap();
    std::fs::copy(std::env::var_os(component).unwrap(), artifact).unwrap();
}

/// Loads every module path through the artifact factory, as an application's module loader does.
pub fn loader() -> ModuleLoader<Local> {
    Rc::new(|path: &str| {
        Box::pin(std::future::ready(Ok(Some(crate::artifact(
            path.to_owned(),
        )))))
    })
}

/// Values the components reported on the `trace` channel, in delivery order.
pub type Trace = Rc<RefCell<Vec<Value>>>;

/// A bus whose `trace` channel the test reads; a tail or listener error fails the test.
pub struct TracedBus {
    /// The bus every loaded component receives.
    pub controller: EventBusController<Local>,
    /// Reported values.
    pub trace: Trace,
    /// Keeps the trace listener registered.
    _subscription: Subscription<Local>,
}

impl TracedBus {
    /// A bus with a trace listener.
    pub fn new() -> Self {
        let controller = create_event_bus_for::<Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no listener errors")),
        );
        let trace = Trace::default();
        let reported = trace.clone();
        let subscription = controller.on(
            "trace",
            Rc::new(move |data, _| {
                reported.borrow_mut().push(Value::clone(&data));
                Ok(None)
            }),
        );
        Self {
            controller,
            trace,
            _subscription: subscription,
        }
    }

    /// Removes and returns the values reported so far.
    pub fn take(&self) -> Vec<Value> {
        self.trace.take()
    }
}

/// Sends one request to the `hooks` command of a loaded hook component.
pub async fn hooks(
    owner: &ExtensionHandle<Local>,
    runner: &ExtensionRunner<Local>,
    request: &Value,
) -> Result<(), String> {
    let handler = {
        let owner = owner.read().unwrap();
        let command = owner.commands["hooks"].read().unwrap();
        command.handler.clone()
    };
    handler(request.to_string(), runner.create_command_context())
        .await
        .map_err(|error| error.to_string())
}
