//! A test-only native host: it instantiates the built author component and implements the
//! imports the component calls.
use std::path::Path;
use std::sync::OnceLock;

use tokio::sync::oneshot;
use wasmtime::component::{Accessor, Component, HasData, Linker, Resource, ResourceTable};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

use super::observed::{Observed, Session, pause};
use crate::bindings::host_side::Extension;
use crate::bindings::host_side::exports::maestro::extension::guest::Guest;
use crate::bindings::host_side::maestro::extension::session::{
    NewSessionCommandData, SessionChangeResult,
};
use crate::bindings::host_side::maestro::extension::{host, session};

/// Host identity of one guest closure.
pub struct Identity(u32);
/// Cancellation flag.
pub struct Flag(bool);

/// Host type of the ordinary context resource.
pub struct Ordinary(Session);
/// Host type of the command context resource.
pub struct Command(Session);
/// Host type of the replacement context resource.
pub struct Replaced(Session);

/// Host state shared by every import.
pub struct State {
    /// WASI state the component's standard library needs.
    wasi: WasiCtx,
    /// Host-side resources by handle.
    pub table: ResourceTable,
    /// What the host observed.
    pub observed: Observed,
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
            observed: Observed::default(),
            guest: None,
        }
    }

    /// Adds a resource to the table and counts it as lent to the extension.
    fn push<T: Send + 'static>(&mut self, value: T) -> wasmtime::Result<Resource<T>> {
        self.observed.lend();
        Ok(self.table.push(value)?)
    }

    /// Removes a resource the component dropped.
    fn release<T: 'static>(&mut self, this: Resource<T>) -> wasmtime::Result<()> {
        self.table.delete(this)?;
        self.observed.reclaim();
        Ok(())
    }
}

impl session::Host for State {}

impl host::HostCallback for State {
    fn new(&mut self) -> wasmtime::Result<Resource<Identity>> {
        let identity = self.observed.next_identity();
        Ok(self.table.push(Identity(identity))?)
    }

    fn id(&mut self, this: Resource<Identity>) -> wasmtime::Result<u32> {
        Ok(self.table.get(&this)?.0)
    }

    fn drop(&mut self, this: Resource<Identity>) -> wasmtime::Result<()> {
        let identity = self.table.delete(this)?.0;
        self.observed.drop_identity(identity);
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
        Ok(self.observed.on(&event, handler.rep()))
    }

    fn register_command(
        &mut self,
        name: String,
        _description: Option<String>,
        handler: Resource<Identity>,
    ) -> wasmtime::Result<Result<(), String>> {
        self.observed.register_command(&name, handler.rep());
        Ok(Ok(()))
    }

    fn append_entry(
        &mut self,
        custom_type: String,
        data: Option<String>,
    ) -> wasmtime::Result<Result<(), String>> {
        self.observed.entry(&custom_type, data.as_deref());
        Ok(Ok(()))
    }
}

/// Selects the host state as the data every import runs against.
struct Imports;

impl HasData for Imports {
    type Data<'a> = &'a mut State;
}

impl Imports {
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
    async fn wait_for_idle(
        accessor: &Accessor<State, Self>,
        this: Resource<Command>,
    ) -> wasmtime::Result<Result<(), String>> {
        let hold = accessor.with(|mut access| -> wasmtime::Result<_> {
            let state = access.get();
            let cwd = state.table.get(&this)?.0.cwd.clone();
            state.observed.log(format!("wait-for-idle {cwd}"));
            Ok(state.observed.take_hold())
        })?;
        pause(hold).await;
        Ok(Ok(()))
    }

    async fn new_session(
        accessor: &Accessor<State, Self>,
        this: Resource<Command>,
        data: NewSessionCommandData,
        with_session: Option<Resource<Identity>>,
    ) -> wasmtime::Result<Result<SessionChangeResult, String>> {
        let parent = data.parent_session.as_deref().unwrap_or("none");
        let line = format!("new-session start parent={parent}");
        accessor.with(|mut access| access.get().observed.log(line));
        if accessor.with(|mut access| access.get().observed.take_rejection()) {
            return Ok(Err("session rejected".to_owned()));
        }
        if let Err(message) = Self::continue_with(accessor, with_session, "/replacement").await? {
            return Ok(Err(message));
        }
        accessor.with(|mut access| -> wasmtime::Result<_> {
            let state = access.get();
            state.table.get(&this)?.0.stale.set(true);
            state.observed.log("new-session done cancelled=false");
            Ok(Ok(SessionChangeResult { cancelled: false }))
        })
    }
}

/// Once the host saw a held wait for idle pending, records that and lets the wait continue; a
/// delivery without a hold has nothing to wait for.
pub async fn release_held(
    accessor: &Accessor<State>,
    held: Option<(oneshot::Receiver<()>, oneshot::Sender<()>)>,
) {
    let Some((pending, proceed)) = held else {
        return;
    };
    let _ = pending.await;
    accessor.with(|mut access| access.get().observed.suspended());
    let _ = proceed.send(());
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

/// The engine and the component compiled from the built artifact; compiling dominates a test
/// run, so every harness of the process shares one.
fn compiled(path: &Path) -> wasmtime::Result<&'static (Engine, Component)> {
    static COMPILED: OnceLock<Result<(Engine, Component), String>> = OnceLock::new();
    COMPILED
        .get_or_init(|| {
            let mut config = Config::new();
            config.wasm_component_model_async(true);
            let engine = Engine::new(&config).map_err(|error| error.to_string())?;
            let component =
                Component::from_file(&engine, path).map_err(|error| error.to_string())?;
            Ok((engine, component))
        })
        .as_ref()
        .map_err(|message| wasmtime::format_err!("{message}"))
}

impl Harness {
    /// Instantiates the component and runs its factory.
    pub async fn start(path: &Path) -> wasmtime::Result<Self> {
        let (engine, component) = compiled(path)?;
        let mut linker = Linker::new(engine);
        wasmtime_wasi::p2::add_to_linker_async(&mut linker)?;
        Extension::add_to_linker::<State, Imports>(&mut linker, |state| state)?;
        let mut store = Store::new(engine, State::new());
        let extension = Extension::instantiate_async(&mut store, component, &linker).await?;
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

    /// The table key a callback was registered under.
    pub fn callback(&self, name: &str) -> wasmtime::Result<u32> {
        self.store
            .data()
            .observed
            .callback(name)
            .map_err(wasmtime::Error::msg)
    }

    /// What the host observed.
    pub fn observed(&mut self) -> &mut Observed {
        &mut self.store.data_mut().observed
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

    /// Asks the component to release the callback at table key `rep`.
    pub async fn release(&mut self, rep: u32) -> wasmtime::Result<()> {
        self.exports
            .func_release()
            .call_async(&mut self.store, (borrow(rep),))
            .await
    }
}
