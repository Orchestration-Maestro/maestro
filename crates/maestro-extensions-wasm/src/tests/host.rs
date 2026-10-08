//! A test-only native host: it instantiates the built author component and implements the
//! imports the component calls.
use std::collections::HashMap;
use std::future::Future;

use tokio::sync::oneshot;
use wasmtime::component::{Accessor, Component, HasData, Linker, Resource, ResourceTable};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

use super::controlled::{Hold, REJECTED, STALE};
use crate::bindings::host_side::Extension;
use crate::bindings::host_side::exports::maestro::extension::guest::Guest;
use crate::bindings::host_side::maestro::extension::session::{
    NewSessionCommandData, SessionChangeResult,
};
use crate::bindings::host_side::maestro::extension::{events, host, models, session};

/// Host identity of one guest closure.
pub struct Identity(u32);
/// Cancellation flag.
pub struct Flag(bool);

/// Session state; each context kind wraps it in its own host type.
pub struct Session {
    /// Working directory of the session.
    cwd: String,
    /// Whether an operation replaced the session.
    stale: bool,
}

/// Host type of the ordinary context resource.
pub struct Ordinary(Session);
/// Host type of the command context resource.
pub struct Command(Session);
/// Host type of the replacement context resource.
pub struct Replaced(Session);

impl Session {
    /// A live session in `cwd`.
    fn new(cwd: &str) -> Self {
        Self {
            cwd: cwd.to_owned(),
            stale: false,
        }
    }

    /// The working directory, or the stale message.
    fn cwd(&self) -> Result<String, String> {
        if self.stale {
            Err(STALE.to_owned())
        } else {
            Ok(self.cwd.clone())
        }
    }
}

/// Host state shared by every import.
pub struct State {
    /// WASI state the component's standard library needs.
    wasi: WasiCtx,
    /// Host-side resources by handle.
    pub table: ResourceTable,
    /// Lines the host observed, in time order.
    pub transcript: Vec<String>,
    /// Identities the component dropped, in drop order.
    pub dropped: Vec<u32>,
    /// Last identity handed out.
    next: u32,
    /// Registered callback identities by key, such as `event input`.
    pub registered: HashMap<String, u32>,
    /// Keys in registration order.
    pub order: Vec<String>,
    /// The hold set for the next session operation.
    hold: Option<Hold>,
    /// Whether the next session operation fails without running its continuation.
    reject_session: bool,
    /// The guest, once instantiated, so imports can call back into it.
    guest: Option<Guest>,
}

impl WasiView for State {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

impl State {
    /// A state with no registrations.
    fn new() -> Self {
        Self {
            wasi: WasiCtxBuilder::new().build(),
            table: ResourceTable::new(),
            transcript: Vec::new(),
            dropped: Vec::new(),
            next: 0,
            registered: HashMap::new(),
            order: Vec::new(),
            hold: None,
            reject_session: false,
            guest: None,
        }
    }

    /// Adds a resource to the table.
    fn push<T: Send + 'static>(&mut self, value: T) -> wasmtime::Result<Resource<T>> {
        Ok(self.table.push(value)?)
    }

    /// Removes a resource the component dropped.
    fn release<T: 'static>(&mut self, this: Resource<T>) -> wasmtime::Result<()> {
        self.table.delete(this)?;
        Ok(())
    }

    /// Appends a line to the transcript.
    fn log(&mut self, line: String) {
        self.transcript.push(line);
    }

    /// Remembers the identity a callback was registered under.
    fn register(&mut self, key: String, callback: &Resource<Identity>) {
        self.order.push(key.clone());
        self.registered.insert(key, callback.rep());
    }
}

impl events::Host for State {}
impl models::Host for State {}
impl session::Host for State {}

impl host::HostCallback for State {
    fn new(&mut self) -> wasmtime::Result<Resource<Identity>> {
        self.next += 1;
        self.push(Identity(self.next))
    }

    fn id(&mut self, this: Resource<Identity>) -> wasmtime::Result<u32> {
        Ok(self.table.get(&this)?.0)
    }

    fn drop(&mut self, this: Resource<Identity>) -> wasmtime::Result<()> {
        self.dropped.push(self.table.delete(this)?.0);
        Ok(())
    }
}

impl host::HostAbortSignal for State {
    fn aborted(&mut self, this: Resource<Flag>) -> wasmtime::Result<bool> {
        Ok(self.table.get(&this)?.0)
    }

    fn drop(&mut self, this: Resource<Flag>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

/// Methods every session-bound context resource shares.
macro_rules! context_methods {
    ($kind:ident) => {
        fn cwd(&mut self, this: Resource<$kind>) -> wasmtime::Result<Result<String, String>> {
            Ok(self.table.get(&this)?.0.cwd())
        }

        fn drop(&mut self, this: Resource<$kind>) -> wasmtime::Result<()> {
            self.release(this)
        }
    };
}

impl host::HostContext for State {
    context_methods!(Ordinary);
}

impl host::HostCommandContext for State {
    context_methods!(Command);
}

impl host::HostReplacedSessionContext for State {
    fn command(&mut self, this: Resource<Replaced>) -> wasmtime::Result<Resource<Command>> {
        let cwd = self.table.get(&this)?.0.cwd.clone();
        self.push(Command(Session::new(&cwd)))
    }

    fn drop(&mut self, this: Resource<Replaced>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

impl host::Host for State {
    fn on(
        &mut self,
        event: String,
        handler: Resource<Identity>,
    ) -> wasmtime::Result<Result<(), String>> {
        if event == REJECTED {
            self.log(format!("reject event {event}"));
            return Ok(Err(format!("registration rejected: {event}")));
        }
        self.log(format!("register event {event}"));
        self.register(format!("event {event}"), &handler);
        Ok(Ok(()))
    }

    fn register_command(
        &mut self,
        name: String,
        _description: Option<String>,
        handler: Resource<Identity>,
    ) -> wasmtime::Result<Result<(), String>> {
        self.log(format!("register command {name}"));
        self.register(format!("command {name}"), &handler);
        Ok(Ok(()))
    }

    fn append_entry(
        &mut self,
        custom_type: String,
        data: Option<String>,
    ) -> wasmtime::Result<Result<(), String>> {
        self.log(format!(
            "entry {custom_type} {}",
            data.as_deref().unwrap_or("none")
        ));
        Ok(Ok(()))
    }
}

/// Selects the host state as the data every import runs against.
struct Imports;

impl HasData for Imports {
    type Data<'a> = &'a mut State;
}

impl Imports {
    /// Reports that an operation is pending and waits until the driver lets it continue.
    async fn pending(accessor: &Accessor<State, Self>) {
        let hold = accessor.with(|mut access| access.get().hold.take());
        if let Some(Hold { started, open }) = hold {
            let _ = started.send(());
            let _ = open.await;
        }
    }

    /// Runs the continuation announced for an operation against a fresh replacement context.
    async fn continue_with(
        accessor: &Accessor<State, Self>,
        callback: Option<Resource<Identity>>,
        cwd: &str,
    ) -> wasmtime::Result<Result<(), String>> {
        let Some(callback) = callback else {
            return Ok(Ok(()));
        };
        let (guest, replaced) = accessor.with(|mut access| -> wasmtime::Result<_> {
            let state = access.get();
            let guest = state
                .guest
                .clone()
                .ok_or_else(|| wasmtime::format_err!("no guest recorded"))?;
            Ok((guest, state.push(Replaced(Session::new(cwd)))?))
        })?;
        guest
            .call_invoke_with_session(accessor, Resource::new_borrow(callback.rep()), replaced)
            .await
    }
}

impl host::HostCommandContextWithStore<State> for Imports {
    fn wait_for_idle(
        accessor: &Accessor<State, Self>,
        this: Resource<Command>,
    ) -> impl Future<Output = wasmtime::Result<Result<(), String>>> + Send {
        std::future::ready(accessor.with(|mut access| -> wasmtime::Result<_> {
            let state = access.get();
            let cwd = state.table.get(&this)?.0.cwd.clone();
            state.log(format!("wait-for-idle {cwd}"));
            Ok(Ok(()))
        }))
    }

    async fn new_session(
        accessor: &Accessor<State, Self>,
        this: Resource<Command>,
        data: NewSessionCommandData,
        with_session: Option<Resource<Identity>>,
    ) -> wasmtime::Result<Result<SessionChangeResult, String>> {
        let parent = data.parent_session.as_deref().unwrap_or("none");
        let line = format!("new-session start parent={parent}");
        accessor.with(|mut access| access.get().log(line));
        Self::pending(accessor).await;
        if accessor.with(|mut access| std::mem::take(&mut access.get().reject_session)) {
            return Ok(Err("session rejected".to_owned()));
        }
        if let Err(message) = Self::continue_with(accessor, with_session, "/replacement").await? {
            return Ok(Err(message));
        }
        accessor.with(|mut access| -> wasmtime::Result<_> {
            let state = access.get();
            state.table.get_mut(&this)?.0.stale = true;
            state.log("new-session done cancelled=false".to_owned());
            Ok(Ok(SessionChangeResult { cancelled: false }))
        })
    }
}

/// Turns a message from the extension into a host error.
pub fn reported<T>(result: Result<T, String>) -> wasmtime::Result<T> {
    result.map_err(wasmtime::Error::msg)
}

/// A borrowed callback identity for the given table key.
pub fn borrow(rep: u32) -> Resource<Identity> {
    Resource::new_borrow(rep)
}

/// The instantiated component and the table keys of what it registered.
pub struct Harness {
    /// The store holding the host state and the instance.
    pub store: Store<State>,
    /// The guest exports.
    pub exports: Guest,
}

impl Harness {
    /// Instantiates the component and runs its factory.
    pub async fn start(path: &std::path::Path) -> wasmtime::Result<Self> {
        let mut config = Config::new();
        config.wasm_component_model_async(true);
        let engine = Engine::new(&config)?;
        let component = Component::from_file(&engine, path)?;
        let mut linker = Linker::new(&engine);
        wasmtime_wasi::p2::add_to_linker_async(&mut linker)?;
        Extension::add_to_linker::<State, Imports>(&mut linker, |state| state)?;
        let mut store = Store::new(&engine, State::new());
        let extension = Extension::instantiate_async(&mut store, &component, &linker).await?;
        let exports = extension.maestro_extension_guest().clone();
        store.data_mut().guest = Some(exports.clone());
        let started = exports.clone();
        reported(
            store
                .run_concurrent(async |accessor| started.call_start(accessor).await)
                .await??,
        )?;
        Ok(Self { store, exports })
    }

    /// The identity a callback was registered under.
    pub fn callback(&self, key: &str) -> wasmtime::Result<u32> {
        self.store
            .data()
            .registered
            .get(key)
            .copied()
            .ok_or_else(|| wasmtime::format_err!("nothing registered as {key}"))
    }

    /// Adds a line to the transcript.
    pub fn note(&mut self, line: String) {
        self.store.data_mut().transcript.push(line);
    }

    /// Keeps the next session operation pending.
    pub fn hold_next_session(&mut self, hold: Hold) {
        self.store.data_mut().hold = Some(hold);
    }

    /// Makes the next session operation fail without running its continuation.
    pub fn reject_next_session(&mut self) {
        self.store.data_mut().reject_session = true;
    }

    /// A fresh ordinary context in `cwd`.
    pub fn ordinary(&mut self, cwd: &str) -> wasmtime::Result<Resource<Ordinary>> {
        self.store.data_mut().push(Ordinary(Session::new(cwd)))
    }

    /// A fresh identity the component never registered.
    pub fn identity(&mut self, id: u32) -> wasmtime::Result<u32> {
        Ok(self.store.data_mut().push(Identity(id))?.rep())
    }

    /// A fresh replacement context in `cwd`.
    pub fn replaced(&mut self, cwd: &str) -> wasmtime::Result<Resource<Replaced>> {
        self.store.data_mut().push(Replaced(Session::new(cwd)))
    }

    /// A fresh command context in `cwd`.
    pub fn command_context(&mut self, cwd: &str) -> wasmtime::Result<Resource<Command>> {
        self.store.data_mut().push(Command(Session::new(cwd)))
    }

    /// A fresh cancellation flag, already cancelled when `aborted`.
    pub fn flag(&mut self, aborted: bool) -> wasmtime::Result<u32> {
        Ok(self.store.data_mut().push(Flag(aborted))?.rep())
    }

    /// Cancels a flag the component may have retained.
    pub fn abort(&mut self, flag: u32) -> wasmtime::Result<()> {
        self.store
            .data_mut()
            .table
            .get_mut(&Resource::<Flag>::new_borrow(flag))?
            .0 = true;
        Ok(())
    }

    /// Asks the component to release the callback registered under `key`.
    pub async fn release(&mut self, key: &str) -> wasmtime::Result<()> {
        let rep = self.callback(key)?;
        self.exports
            .func_release()
            .call_async(&mut self.store, (borrow(rep),))
            .await
    }
}

/// Channels that hold a new session pending until the driver saw it.
pub fn gate() -> (Hold, oneshot::Receiver<()>, oneshot::Sender<()>) {
    let (started_tx, started) = oneshot::channel();
    let (open, opened) = oneshot::channel();
    (
        Hold {
            started: started_tx,
            open: opened,
        },
        started,
        open,
    )
}
