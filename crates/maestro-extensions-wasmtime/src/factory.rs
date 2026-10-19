//! Artifact initialization uses the existing native factory and its original facade.
use crate::{
    guest_calls::{Call, Callbacks},
    lifecycle::{self, replies::Replies},
    local_resources::Pump,
};
use maestro_extensions::{ConfigValueOperations, ExtensionFactory, Local};
use std::{rc::Rc, sync::Arc};
use tokio::sync::mpsc;
use wasmtime::{
    Config, Engine, Store,
    component::{Component, Linker},
};

/// Creates a local factory for one authored extension component at the supplied path.
///
/// Credential effects are supplied by the caller; construction performs no discovery or I/O.
/// The factory requires a Tokio local task set to keep native capabilities on their owner.
#[must_use]
pub fn artifact_factory(
    path: String,
    credential_operations: Rc<dyn ConfigValueOperations>,
) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let path = path.clone();
        let credential_operations = credential_operations.clone();
        Box::pin(async move {
            let (pump, admission) = Pump::new(credential_operations);
            let endpoint = pump.registry().api(api)?;
            let (calls, mut incoming) = mpsc::unbounded_channel();
            let replies = Replies::default();
            let callbacks = Callbacks::new(calls.downgrade(), replies.clone());
            let weak = calls.downgrade();
            let startup = lifecycle::StoreEndpoint::new(calls, admission, replies.clone());
            let state = lifecycle::State::new(endpoint, callbacks);
            std::thread::Builder::new()
                .name("maestro-extension".into())
                .spawn(move || {
                    worker(&path, state, &mut incoming, weak, &replies);
                })
                .map_err(|error| error.to_string())?;
            tokio::task::spawn_local(pump.serve());
            startup.submit(Call::Start).await.map_err(Into::into)
        })
    })
}

/// Owns the Store until native registrations, work and queued final releases end.
fn worker(
    path: &str,
    state: lifecycle::State,
    incoming: &mut mpsc::UnboundedReceiver<Call>,
    calls: mpsc::WeakUnboundedSender<Call>,
    replies: &Replies,
) {
    let runtime = tokio::runtime::Builder::new_current_thread().build();
    match runtime {
        Ok(runtime) => runtime.block_on(async {
            if let Err(error) = execute(path, state, incoming, calls, replies).await {
                replies.fault(format!("component fault {path}: {error:#}"));
                lifecycle::faulted(incoming).await;
            }
        }),
        Err(error) => {
            replies.fault(format!("component fault {path}: {error}"));
        }
    }
}

/// Instantiates the canonical world and enters its one production lifecycle.
async fn execute(
    path: &str,
    state: lifecycle::State,
    incoming: &mut mpsc::UnboundedReceiver<Call>,
    calls: mpsc::WeakUnboundedSender<Call>,
    replies: &Replies,
) -> wasmtime::Result<()> {
    let mut config = Config::new();
    config.wasm_component_model_async(true);
    let engine = Engine::new(&config)?;
    let component = Component::from_file(&engine, path)?;
    let mut linker = Linker::new(&engine);
    lifecycle::link(&mut linker)?;
    let mut store = Store::new(&engine, state);
    let instance = linker.instantiate_async(&mut store, &component).await?;
    let exports = Arc::new(crate::guest_exports::Exports::new(&mut store, &instance)?);
    if let Err(error) = lifecycle::drive(&mut store, exports, incoming, calls).await {
        replies.fault(format!("component fault {path}: {error:#}"));
        lifecycle::faulted(incoming).await;
    }
    Ok(())
}
