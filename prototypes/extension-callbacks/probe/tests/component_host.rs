//! Instantiates the example component in a test-only host and checks the shared transcript.

use tokio::sync::oneshot;
use wasmtime::component::{Accessor, Component, HasData, Linker, Resource, ResourceTable};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

#[path = "../../shared/scenario.rs"]
mod scenario;

/// Path of the built example component.
const COMPONENT: &str = "../target/wasm32-wasip2/debug/extension_example_component.wasm";
/// Message a context reports after its session was replaced.
const STALE: &str = "This context is stale after session replacement.";

/// Host identity of one guest closure.
pub struct Identity(u32);
/// Cancellation flag.
pub struct Flag(bool);
/// Progress channel of one tool call.
pub struct Progress(String);
/// Session state; each context kind wraps it in its own host type.
pub struct Session {
    cwd: String,
    stale: bool,
}
/// Host type of the ordinary context resource.
pub struct Ordinary(Session);
/// Host type of the command context resource.
pub struct Command(Session);
/// Host type of the replacement context resource.
pub struct Replaced(Session);

wasmtime::component::bindgen!({
    path: "../wit",
    world: "extension",
    imports: { default: trappable },
    with: {
        "maestro:extension/types.callback": Identity,
        "maestro:extension/types.abort-signal": Flag,
        "maestro:extension/types.tool-update": Progress,
        "maestro:extension/types.context": Ordinary,
        "maestro:extension/types.command-context": Command,
        "maestro:extension/types.replaced-session-context": Replaced,
    },
});
use exports::maestro::extension::guest::Guest;
use maestro::extension::{host, types};

/// Table keys of the callbacks the extension registered.
#[derive(Default, Clone, Copy)]
struct Registered {
    input: u32,
    prepare: u32,
    execute: u32,
    command: u32,
    renderer: u32,
}

/// Lets the driver observe a pending new session and release it.
struct Hold {
    started: oneshot::Sender<()>,
    open: oneshot::Receiver<()>,
}

/// Host state shared by every import.
struct State {
    wasi: WasiCtx,
    table: ResourceTable,
    transcript: Vec<String>,
    dropped: Vec<u32>,
    next: u32,
    registered: Registered,
    hold: Option<Hold>,
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

impl Session {
    /// A live session working in `cwd`.
    fn new(cwd: &str) -> Self {
        Self {
            cwd: cwd.to_owned(),
            stale: false,
        }
    }

    /// The directory, or the stale-context message.
    fn cwd(&self) -> Result<String, String> {
        if self.stale {
            Err(STALE.to_owned())
        } else {
            Ok(self.cwd.clone())
        }
    }
}

impl State {
    /// Empty host state.
    fn new() -> Self {
        Self {
            wasi: WasiCtxBuilder::new().build(),
            table: ResourceTable::new(),
            transcript: Vec::new(),
            dropped: Vec::new(),
            next: 0,
            registered: Registered::default(),
            hold: None,
            guest: None,
        }
    }

    /// Adds a resource to the table.
    fn push<T: Send + 'static>(&mut self, value: T) -> wasmtime::Result<Resource<T>> {
        Ok(self.table.push(value)?)
    }

    /// Removes a resource from the table.
    fn release<T: 'static>(&mut self, this: Resource<T>) -> wasmtime::Result<()> {
        self.table.delete(this)?;
        Ok(())
    }

    /// Records one observed line.
    fn log(&mut self, line: String) {
        self.transcript.push(line);
    }
}

impl types::Host for State {}

impl types::HostCallback for State {
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

impl types::HostAbortSignal for State {
    fn aborted(&mut self, this: Resource<Flag>) -> wasmtime::Result<bool> {
        Ok(self.table.get(&this)?.0)
    }

    fn drop(&mut self, this: Resource<Flag>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

impl types::HostToolUpdate for State {
    fn send(
        &mut self,
        this: Resource<Progress>,
        partial: types::ToolResult,
    ) -> wasmtime::Result<Result<(), String>> {
        let call = self.table.get(&this)?.0.clone();
        self.log(format!("tool-update {call} {}", partial.content.join(",")));
        Ok(Ok(()))
    }

    fn drop(&mut self, this: Resource<Progress>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

/// Methods every session-bound context resource shares.
macro_rules! session_methods {
    ($kind:ident) => {
        fn cwd(&mut self, this: Resource<$kind>) -> wasmtime::Result<Result<String, String>> {
            Ok(self.table.get(&this)?.0.cwd())
        }

        fn signal(
            &mut self,
            _: Resource<$kind>,
        ) -> wasmtime::Result<Result<Option<Resource<Flag>>, String>> {
            Ok(Ok(None))
        }

        fn drop(&mut self, this: Resource<$kind>) -> wasmtime::Result<()> {
            self.release(this)
        }
    };
}

impl types::HostContext for State {
    session_methods!(Ordinary);
}

impl types::HostCommandContext for State {
    session_methods!(Command);
}

impl types::HostReplacedSessionContext for State {
    fn command(&mut self, this: Resource<Replaced>) -> wasmtime::Result<Resource<Command>> {
        let cwd = self.table.get(&this)?.0.cwd.clone();
        self.push(Command(Session::new(&cwd)))
    }

    fn drop(&mut self, this: Resource<Replaced>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

impl host::Host for State {
    fn register_tool(
        &mut self,
        metadata: types::ToolMetadata,
        prepare: Option<Resource<Identity>>,
        execute: Resource<Identity>,
    ) -> wasmtime::Result<Result<(), String>> {
        self.log(format!(
            "register tool {} prepare={}",
            metadata.name,
            prepare.is_some()
        ));
        self.registered.prepare = prepare.map_or(0, |callback| callback.rep());
        self.registered.execute = execute.rep();
        Ok(Ok(()))
    }

    fn on_input(&mut self, handler: Resource<Identity>) -> wasmtime::Result<Result<(), String>> {
        self.log("register input".to_owned());
        self.registered.input = handler.rep();
        Ok(Ok(()))
    }

    fn register_command(
        &mut self,
        name: String,
        handler: Resource<Identity>,
    ) -> wasmtime::Result<Result<(), String>> {
        self.log(format!("register command {name}"));
        self.registered.command = handler.rep();
        Ok(Ok(()))
    }

    fn register_message_renderer(
        &mut self,
        custom_type: String,
        renderer: Resource<Identity>,
    ) -> wasmtime::Result<Result<(), String>> {
        self.log(format!("register renderer {custom_type}"));
        self.registered.renderer = renderer.rep();
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

impl types::HostCommandContextWithStore<State> for Imports {
    async fn new_session(
        accessor: &Accessor<State, Self>,
        this: Resource<Command>,
        options: types::NewSessionOptions,
        with_session: Option<Resource<Identity>>,
    ) -> wasmtime::Result<Result<types::SessionChangeResult, String>> {
        let parent = options.parent_session.as_deref().unwrap_or("none");
        let line = format!("new-session start parent={parent}");
        let (guest, hold) = accessor.with(|mut access| -> wasmtime::Result<_> {
            let state = access.get();
            state.log(line);
            let guest = state
                .guest
                .clone()
                .ok_or_else(|| wasmtime::format_err!("no guest recorded"))?;
            Ok((guest, state.hold.take()))
        })?;
        if let Some(Hold { started, open }) = hold {
            let _ = started.send(());
            let _ = open.await;
        }
        let replaced = accessor
            .with(|mut access| access.get().push(Replaced(Session::new("/replacement"))))?;
        if let Some(callback) = with_session {
            let callback = Resource::new_borrow(callback.rep());
            if let Err(message) = guest
                .call_invoke_with_session(accessor, callback, replaced)
                .await?
            {
                return Ok(Err(message));
            }
        }
        accessor.with(|mut access| -> wasmtime::Result<_> {
            let state = access.get();
            state.table.get_mut(&this)?.0.stale = true;
            state.log("new-session done cancelled=false".to_owned());
            Ok(())
        })?;
        Ok(Ok(types::SessionChangeResult { cancelled: false }))
    }
}

impl types::HostReplacedSessionContextWithStore<State> for Imports {
    fn send_user_message(
        accessor: &Accessor<State, Self>,
        this: Resource<Replaced>,
        text: String,
    ) -> impl Future<Output = wasmtime::Result<Result<(), String>>> + Send {
        let logged = accessor.with(|mut access| -> wasmtime::Result<_> {
            let state = access.get();
            let cwd = state.table.get(&this)?.0.cwd.clone();
            state.log(format!("user-message {cwd} {text}"));
            Ok(Ok(()))
        });
        std::future::ready(logged)
    }
}

/// Turns a message from the extension into a host error.
fn reported<T>(result: Result<T, String>) -> wasmtime::Result<T> {
    result.map_err(wasmtime::Error::msg)
}

/// A borrowed callback identity for the given table key.
fn borrow(rep: u32) -> Resource<Identity> {
    Resource::new_borrow(rep)
}

/// The instantiated component and what the host knows about it.
struct Harness {
    store: Store<State>,
    exports: Guest,
    registered: Registered,
}

impl Harness {
    /// Instantiates the component and runs its factory.
    async fn start() -> wasmtime::Result<Self> {
        let mut config = Config::new();
        config.wasm_component_model_async(true);
        let engine = Engine::new(&config)?;
        let component = Component::from_file(&engine, COMPONENT)?;
        let mut linker = Linker::new(&engine);
        wasmtime_wasi::p2::add_to_linker_async(&mut linker)?;
        Extension::add_to_linker::<State, Imports>(&mut linker, |state| state)?;
        let mut store = Store::new(&engine, State::new());
        let extension = Extension::instantiate_async(&mut store, &component, &linker).await?;
        let exports = extension.maestro_extension_guest().clone();
        store.data_mut().guest = Some(exports.clone());
        reported(
            store
                .run_concurrent(async |accessor| exports.call_start(accessor).await)
                .await??,
        )?;
        let registered = std::mem::take(&mut store.data_mut().registered);
        Ok(Self {
            store,
            exports,
            registered,
        })
    }

    /// Records a line the driver observed.
    fn note(&mut self, line: String) {
        self.store.data_mut().transcript.push(line);
    }

    /// Dispatches one input and records the outcome.
    async fn input(&mut self, text: &str) -> wasmtime::Result<()> {
        let (exports, handler) = (&self.exports, self.registered.input);
        let outcome = self
            .store
            .run_concurrent(async |accessor| {
                let ctx = accessor
                    .with(|mut access| access.get().push(Ordinary(Session::new("/work"))))?;
                let event = types::InputEvent {
                    text: text.to_owned(),
                    images: None,
                    source: types::InputSource::Interactive,
                };
                exports
                    .call_invoke_input(accessor, borrow(handler), event, ctx)
                    .await
            })
            .await??;
        let decision = match outcome.decision {
            Ok(Some(types::InputResult::Transform(replacement))) => {
                format!("transform:{}", replacement.text)
            }
            Ok(_) => "other".to_owned(),
            Err(message) => format!("error:{message}"),
        };
        self.note(format!(
            "input-outcome text={} decision={decision}",
            outcome.event.text
        ));
        Ok(())
    }

    /// Prepares tool arguments.
    async fn prepare(&mut self, raw: &str) -> wasmtime::Result<String> {
        let prepare = self.exports.func_prepare_arguments();
        let (prepared,) = prepare
            .call_async(&mut self.store, (borrow(self.registered.prepare), raw))
            .await?;
        let prepared = reported(prepared)?;
        self.note(format!("prepared {prepared}"));
        Ok(prepared)
    }

    /// Runs one tool call with a fresh signal and returns the signal's table key.
    async fn tool(&mut self, call_id: &str, params: &str) -> wasmtime::Result<u32> {
        let flag = self.store.data_mut().push(Flag(false))?.rep();
        let (exports, execute) = (&self.exports, self.registered.execute);
        let result = self
            .store
            .run_concurrent(async |accessor| {
                let (update, ctx) = accessor.with(|mut access| {
                    let state = access.get();
                    Ok::<_, wasmtime::Error>((
                        state.push(Progress(call_id.to_owned()))?,
                        state.push(Ordinary(Session::new("/work")))?,
                    ))
                })?;
                let signal = Resource::<Flag>::new_own(flag);
                let call = exports.call_invoke_tool(
                    accessor,
                    borrow(execute),
                    call_id.to_owned(),
                    params.to_owned(),
                    Some(signal),
                    Some(update),
                    ctx,
                );
                call.await
            })
            .await??;
        let result = reported(result)?;
        let details = result.details.unwrap_or_default();
        self.note(format!(
            "tool-result {call_id} content={} details={details}",
            result.content.join(",")
        ));
        Ok(flag)
    }

    /// Flips a signal after the callback that received it returned.
    fn abort(&mut self, flag: u32) -> wasmtime::Result<()> {
        self.store
            .data_mut()
            .table
            .get_mut(&Resource::<Flag>::new_borrow(flag))?
            .0 = true;
        Ok(())
    }

    /// Runs the replace command, holding its new session open until the host observed it pending.
    async fn command(&mut self) -> wasmtime::Result<()> {
        let (started_tx, started) = oneshot::channel();
        let (open, opened) = oneshot::channel();
        self.store.data_mut().hold = Some(Hold {
            started: started_tx,
            open: opened,
        });
        let (exports, command) = (&self.exports, self.registered.command);
        let finished = self
            .store
            .run_concurrent(async |accessor| {
                let ctx = accessor
                    .with(|mut access| access.get().push(Command(Session::new("/work"))))?;
                let call =
                    exports.call_invoke_command(accessor, borrow(command), "go".to_owned(), ctx);
                let driver = async {
                    started.await?;
                    let last = accessor.with(|mut access| access.get().transcript.last().cloned());
                    assert_eq!(last.as_deref(), Some("new-session start parent=parent"));
                    open.send(())
                        .map_err(|()| wasmtime::format_err!("the host stopped waiting"))
                };
                let (finished, gate) = tokio::join!(call, driver);
                gate.and(finished)
            })
            .await??;
        reported(finished)
    }

    /// Renders the note message, invalidates it and drops the component.
    async fn render(&mut self) -> wasmtime::Result<()> {
        let message = types::CustomMessage {
            custom_type: "note".to_owned(),
            content: "body".to_owned(),
            details: None,
        };
        let render = self.exports.func_invoke_renderer();
        let (component,) = render
            .call_async(
                &mut self.store,
                (borrow(self.registered.renderer), &message, true),
            )
            .await?;
        let Some(component) = reported(component)? else {
            return Err(wasmtime::format_err!("the renderer returned no component"));
        };
        let (lines,) = self
            .exports
            .component()
            .func_render()
            .call_async(&mut self.store, (component, 20))
            .await?;
        self.note(format!("rendered {}", lines.join(",")));
        self.exports
            .component()
            .func_invalidate()
            .call_async(&mut self.store, (component,))
            .await?;
        component.resource_drop_async(&mut self.store).await
    }

    /// Releases every registered callback in registration order.
    async fn release_all(&mut self) -> wasmtime::Result<()> {
        let Registered {
            input,
            prepare,
            execute,
            command,
            renderer,
        } = self.registered;
        for rep in [input, prepare, execute, command, renderer] {
            self.exports
                .func_release()
                .call_async(&mut self.store, (borrow(rep),))
                .await?;
        }
        Ok(())
    }
}

/// The scenario both adapters run.
async fn scenario() -> wasmtime::Result<()> {
    let mut host = Harness::start().await?;
    for text in ["hello", "reject"] {
        host.input(text).await?;
    }
    let prepared = host.prepare(r#"{"text":"x"}"#).await?;
    let first = host.tool("c1", &prepared).await?;
    host.abort(first)?;
    host.tool("c2", &prepared).await?;
    host.command().await?;
    host.render().await?;
    host.release_all().await?;
    let state = host.store.data();
    assert_eq!(state.transcript, scenario::EXPECTED);
    assert_eq!(
        state.dropped,
        [6, 1, 2, 3, 4, 5],
        "the continuation callback is released first, then each registered one"
    );
    Ok(())
}

/// The component produces the transcript the controlled adapter produces.
#[test]
fn component_matches_the_controlled_transcript() -> wasmtime::Result<()> {
    tokio::runtime::Builder::new_current_thread()
        .build()?
        .block_on(scenario())
}
