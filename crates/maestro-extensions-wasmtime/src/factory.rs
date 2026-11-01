//! Artifact initialization uses the existing native factory and its original facade.
use crate::{
    component_engine,
    guest_calls::{Call, Callbacks},
    lifecycle::{self, replies::Replies},
    local_resources::Pump,
};
use maestro_extensions::{ExtensionFactory, ExtensionWidgets, Local};
use std::{rc::Rc, sync::Arc};
use tokio::sync::mpsc;
use wasmtime::{
    Cache, Store,
    component::{Component, Linker},
};

/// Application-supplied facts shared by the components it loads.
#[derive(Debug, Clone)]
pub struct Installation {
    /// Version of the installed application, not the component.
    pub version: String,
    /// Agent directory selected by the application's configuration owner.
    pub agent_dir: String,
}

/// Creates a local factory for one authored extension component at the supplied path, whose
/// host widgets `widgets` builds.
///
/// The application supplies its version and agent directory; components do not choose them.
/// Credential effects are supplied by the caller; construction performs no discovery or I/O.
/// The factory requires a Tokio local task set to keep native capabilities on their owner.
#[must_use]
pub fn artifact_factory(
    path: String,
    widgets: Rc<dyn ExtensionWidgets>,
    installation: Installation,
) -> ExtensionFactory<Local> {
    factory(path, widgets, installation, None, None)
}

/// Creates an artifact factory whose standard output uses the supplied byte writer, and whose
/// host widgets `widgets` builds.
///
/// The writer is shared across initialization and later callbacks on the Store thread.
/// Closing one guest output view does not close the caller's sink. The caller owns
/// terminal lifecycle and supplies the `terminal` observation. Other streams stay inherited.
#[must_use]
pub fn artifact_factory_with_stdout(
    path: String,
    widgets: Rc<dyn ExtensionWidgets>,
    installation: Installation,
    stdout: Box<dyn std::io::Write + Send>,
    terminal: bool,
) -> ExtensionFactory<Local> {
    factory(
        path,
        widgets,
        installation,
        Some(crate::Stdout::new(stdout, terminal)),
        None,
    )
}

/// Creates the production component factory with application-supplied native effects, whose
/// host widgets `widgets` builds.
///
/// The application supplies installation facts and the operations' original local executor.
/// Construction performs no native launch or request; guest calls select their effects.
/// Supplied stdout is retained beside native effects for initialization and callbacks.
/// With no supplied writer, stdout stays inherited.
#[must_use]
pub fn artifact_factory_with_io(
    path: String,
    widgets: Rc<dyn ExtensionWidgets>,
    installation: Installation,
    io: Rc<dyn maestro_extensions::native_io::NativeIo>,
    stdout: Option<crate::Stdout>,
) -> ExtensionFactory<Local> {
    factory(path, widgets, installation, stdout, Some(io))
}

/// Retains the original supplied widgets, output and native effects for each independently
/// loaded component.
fn factory(
    path: String,
    widgets: Rc<dyn ExtensionWidgets>,
    installation: Installation,
    output: Option<crate::Stdout>,
    io: Option<Rc<dyn maestro_extensions::native_io::NativeIo>>,
) -> ExtensionFactory<Local> {
    Rc::new(move |api| {
        let (path, widgets) = (path.clone(), widgets.clone());
        let output = output.clone();
        let io = io.clone();
        let installation = installation.clone();
        Box::pin(async move {
            let (pump, admission) = Pump::new(widgets);
            pump.registry().resources.borrow_mut().native_io = io;
            let endpoint = pump.registry().api(api)?;
            let (calls, incoming) = mpsc::unbounded_channel();
            let shutdown = Arc::new(tokio::sync::Notify::new());
            let retirement = Arc::downgrade(&shutdown);
            let mut incoming = lifecycle::Incoming::new(incoming, shutdown);
            let replies = Replies::default();
            let callbacks = Callbacks::new(calls.downgrade(), replies.clone());
            let weak = calls.downgrade();
            let startup = lifecycle::StoreEndpoint::new(calls, admission, replies.clone());
            let state = lifecycle::State::new(endpoint, callbacks, &path, installation, output)
                .map_err(|error| error.to_string())?;
            let worker = std::thread::Builder::new()
                .name("maestro-extension".into())
                .spawn(move || {
                    worker(&path, state, &mut incoming, weak, &replies);
                })
                .map_err(|error| error.to_string())?;
            pump.own_worker(worker, retirement);
            tokio::task::spawn_local(pump.serve());
            startup.submit(Call::Start).await.map_err(Into::into)
        })
    })
}

/// Owns the Store until native registrations, work and queued final releases end.
fn worker(
    path: &str,
    state: lifecycle::State,
    incoming: &mut lifecycle::Incoming,
    calls: mpsc::WeakUnboundedSender<Call>,
    replies: &Replies,
) {
    // Standard I/O uses this Store thread's existing reactor and timer.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build();
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
    incoming: &mut lifecycle::Incoming,
    calls: mpsc::WeakUnboundedSender<Call>,
    replies: &Replies,
) -> wasmtime::Result<()> {
    // The user cache is optional; unavailable storage must not prevent loading.
    let engine = component_engine(Cache::from_file(None).ok())?;
    let component = Component::from_file(&engine, path)?;
    let mut linker = Linker::new(&engine);
    lifecycle::link(&mut linker)?;
    let mut store = Store::new(&engine, state);
    let instance = linker.instantiate_async(&mut store, &component).await?;
    let exports = Arc::new(crate::guest_exports::Exports::new(&mut store, &instance)?);
    store.data_mut().exports = Some(exports.clone());
    if let Err(error) = lifecycle::drive(&mut store, exports, incoming, calls).await {
        replies.fault(format!("component fault {path}: {error:#}"));
        lifecycle::faulted(incoming).await;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
