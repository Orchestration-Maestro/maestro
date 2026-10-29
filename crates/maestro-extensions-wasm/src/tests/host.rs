//! A test-only native host: it instantiates the built author component and implements the
//! imports the component calls.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, Mutex, OnceLock};
use std::time::Duration;

use tokio::sync::oneshot;
use wasmtime::component::{Accessor, Component, HasData, Linker, Resource, ResourceTable};
use wasmtime::{Engine, Store};
use wasmtime_wasi::clocks::HostWallClock;
use wasmtime_wasi::p2::pipe::MemoryOutputPipe;
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

use super::observed::{Observed, Session, pause};
use crate::bindings::host_side::Extension;
use crate::bindings::host_side::exports::maestro::extension::guest::Guest;
use crate::bindings::host_side::maestro::extension::session::{
    NewSessionCommandData, SessionChangeResult,
};
use crate::bindings::host_side::maestro::extension::{host, session};

/// Snapshot key and registration-ordered callback identities in the author-only host.
type ListenerSnapshot = (u32, Vec<Option<u32>>);

/// Host identity of one guest closure.
pub struct Identity(
    pub(crate) u32,
    pub(crate) Option<super::schema_host::Origin>,
);
pub use super::schema_host::JsonNode;
/// Cancellation flag.
pub struct Flag(pub std::sync::Arc<std::sync::atomic::AtomicBool>);
/// Progress resource owned by one execution.
pub struct Update;

/// Native tool resources are created only by the production runtime, not this fixture.
pub enum BuiltinTool {}
/// Local shell operations exist only on the production host.
pub enum LocalBashOperations {}
impl host::HostLocalBashOperations for State {
    fn drop(&mut self, _: Resource<LocalBashOperations>) -> wasmtime::Result<()> {
        wasmtime::bail!("local shell operations require the production host")
    }
}
impl host::HostLocalBashOperationsWithStore<State> for Imports {
    fn exec(
        _: &Accessor<State, Self>,
        _: Resource<LocalBashOperations>,
        _: host::BashRequest,
        _: Option<Resource<Flag>>,
        _: Resource<Identity>,
    ) -> impl std::future::Future<Output = wasmtime::Result<Result<Option<f64>, String>>> {
        std::future::ready(Err(wasmtime::format_err!(
            "local shell operations require the production host"
        )))
    }
}
/// Queue futures exist only on the production host.
pub enum FileMutation {}
impl host::HostFileMutation for State {
    fn drop(&mut self, _: Resource<FileMutation>) -> wasmtime::Result<()> {
        wasmtime::bail!("file mutation queues require the production host")
    }
}
impl host::HostFileMutationWithStore<State> for Imports {
    fn wait(
        _: &Accessor<State, Self>,
        _: Resource<FileMutation>,
    ) -> impl std::future::Future<Output = wasmtime::Result<Result<(), String>>> {
        std::future::ready(Err(wasmtime::format_err!(
            "file mutation queues require the production host"
        )))
    }
}

/// Native row resources are lent only by the production host.
pub enum NativeRow {}
/// Native component resources are returned only by the production host.
pub enum NativeComponent {}
impl host::HostNativeRow for State {
    fn drop(&mut self, _: Resource<NativeRow>) -> wasmtime::Result<()> {
        wasmtime::bail!("native rendering requires the production host")
    }
}
impl host::HostNativeComponent for State {
    fn render(
        &mut self,
        _: Resource<NativeComponent>,
        _: u32,
    ) -> wasmtime::Result<Resource<Route>> {
        wasmtime::bail!("native rendering requires the production host")
    }
    fn handle_input(
        &mut self,
        _: Resource<NativeComponent>,
        _: String,
    ) -> wasmtime::Result<Resource<Route>> {
        wasmtime::bail!("native rendering requires the production host")
    }
    fn invalidate(&mut self, _: Resource<NativeComponent>) -> wasmtime::Result<Resource<Route>> {
        wasmtime::bail!("native rendering requires the production host")
    }
    fn wants_key_release(
        &mut self,
        _: Resource<NativeComponent>,
    ) -> wasmtime::Result<Resource<Route>> {
        wasmtime::bail!("native rendering requires the production host")
    }
    fn drop(&mut self, _: Resource<NativeComponent>) -> wasmtime::Result<()> {
        wasmtime::bail!("native rendering requires the production host")
    }
}
impl host::HostBuiltinTool for State {
    fn render_call(
        &mut self,
        _: Resource<BuiltinTool>,
        _: Option<String>,
        _: host::ToolRenderInfo,
        _: Resource<super::renderer_host::Theme>,
        _: Option<Resource<NativeRow>>,
        _: Option<Resource<NativeComponent>>,
        _: Option<Resource<Identity>>,
    ) -> wasmtime::Result<Resource<Route>> {
        wasmtime::bail!("native rendering requires the production host")
    }
    fn render_result(
        &mut self,
        _: Resource<BuiltinTool>,
        _: String,
        _: host::ToolRenderResultOptions,
        _: host::ToolRenderInfo,
        _: Resource<super::renderer_host::Theme>,
        _: Option<Resource<NativeRow>>,
        _: Option<Resource<NativeComponent>>,
        _: Option<Resource<Identity>>,
    ) -> wasmtime::Result<Resource<Route>> {
        wasmtime::bail!("native rendering requires the production host")
    }

    fn metadata(&mut self, _: Resource<BuiltinTool>) -> wasmtime::Result<Result<String, String>> {
        wasmtime::bail!("built-in tools require the production host")
    }
    fn prepare(
        &mut self,
        _: Resource<BuiltinTool>,
        _: String,
    ) -> wasmtime::Result<Result<host::PreparedArguments, String>> {
        wasmtime::bail!("built-in tools require the production host")
    }
    fn drop(&mut self, _: Resource<BuiltinTool>) -> wasmtime::Result<()> {
        wasmtime::bail!("built-in tools require the production host")
    }
}
impl host::HostBuiltinToolWithStore<State> for Imports {
    fn execute(
        _: &Accessor<State, Self>,
        _: Resource<BuiltinTool>,
        _: String,
        _: String,
        _: Option<String>,
        _: Option<Resource<Flag>>,
        _: Option<Resource<Identity>>,
    ) -> impl std::future::Future<Output = wasmtime::Result<Result<String, String>>> {
        std::future::ready(Err(wasmtime::format_err!(
            "built-in tools require the production host"
        )))
    }
}

/// Host type of the ordinary context resource.
pub struct Ordinary(Session, super::reader_host::Reader);
/// Host type of the command context resource.
pub struct Command(
    pub(super) Session,
    super::reader_host::Reader,
    Option<super::reader_host::Reader>,
);
/// Host type of the replacement context resource.
pub struct Replaced(
    pub(super) Session,
    pub(super) Option<super::reader_host::Reader>,
);

/// The wall clock of the component's WASI context, set by the test and counting its reads.
#[derive(Clone, Default)]
pub struct Clock(Arc<ClockFace>);

/// State shared by every clone of one supplied clock.
#[derive(Default)]
struct ClockFace {
    /// Milliseconds since the Unix epoch the clock reports.
    millis: AtomicU64,
    /// Wall-clock reads since the last reset.
    reads: AtomicUsize,
}

impl Clock {
    /// Supplies the instant every later read reports.
    pub fn set(&self, millis: u64) {
        self.0.millis.store(millis, Ordering::SeqCst);
    }

    /// Counts reads from zero from now on.
    pub fn reset_reads(&self) {
        self.0.reads.store(0, Ordering::SeqCst);
    }

    /// The wall-clock reads since the last reset.
    pub fn reads(&self) -> usize {
        self.0.reads.load(Ordering::SeqCst)
    }
}

impl HostWallClock for Clock {
    fn resolution(&self) -> Duration {
        Duration::from_millis(1)
    }

    fn now(&self) -> Duration {
        self.0.reads.fetch_add(1, Ordering::SeqCst);
        Duration::from_millis(self.0.millis.load(Ordering::SeqCst))
    }
}

/// Host state shared by every import.
pub struct State {
    /// WASI state the component's standard library needs.
    wasi: WasiCtx,
    /// The wall clock the standard library reads, when the test supplies one.
    clock: Option<Clock>,
    /// Host-side resources by handle.
    pub table: ResourceTable,
    /// Reports delivery of caller-owned cancellation to a pending component dialog.
    pub abort_notice: Option<oneshot::Sender<()>>,
    /// What the host observed.
    pub observed: Observed,
    /// The guest, once instantiated, so imports can call back into it.
    pub(super) guest: Option<Guest>,
    /// The originating instance associated with the current import projection.
    pub(super) schema_origin: Option<Arc<OnceLock<Guest>>>,
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
    /// Records literal emission data and retains this fixture's all-local listener list.
    fn snapshot_listeners(
        &mut self,
        emitter: &Resource<super::bus_host::Emitter>,
        channel: &str,
        data: &str,
    ) -> wasmtime::Result<Result<ListenerSnapshot, String>> {
        let key = self.table.get(emitter)?.key;
        if key != 0 {
            self.observed.bus.renderer_emissions.push((
                key,
                channel.into(),
                serde_json::from_str(data)?,
            ));
        }
        let handlers = match self.observed.bus.emit(channel, data) {
            Ok(handlers) => handlers,
            Err(error) => return Ok(Err(error)),
        };
        let id = u32::try_from(self.observed.bus.snapshots.len())?;
        let tags = handlers
            .iter()
            .map(|id| {
                self.table
                    .get(&borrow(*id))
                    .map(|identity| Some(identity.0))
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.observed.bus.snapshots.push(Some(handlers));
        Ok(Ok((id, tags)))
    }
    /// Provides an independently owned emitter resource for one local prefix.
    fn begin_local_listener(
        &mut self,
        emitter: &Resource<super::bus_host::Emitter>,
        snapshot: u32,
        index: u32,
    ) -> wasmtime::Result<Result<Resource<super::bus_host::Emitter>, String>> {
        let valid = self
            .observed
            .bus
            .snapshots
            .get(snapshot as usize)
            .and_then(Option::as_ref)
            .and_then(|entries| entries.get(index as usize))
            .is_some();
        if !valid {
            return Ok(Err("snapshot entry ended".into()));
        }
        let key = self.table.get(emitter)?.key;
        Ok(Ok(self.table.push(super::bus_host::Emitter { key })?))
    }

    /// A state with no registrations, reading the host's own wall clock.
    fn new() -> Self {
        Self::with_clock(None)
    }

    /// A state with no registrations whose standard library reads `clock`.
    fn with_clock(clock: Option<Clock>) -> Self {
        Self::with_wasi(clock, WasiCtxBuilder::new())
    }

    /// A state whose standard output the guest writes into `stdout`.
    fn capturing(stdout: &MemoryOutputPipe) -> Self {
        let mut builder = WasiCtxBuilder::new();
        builder.stdout(stdout.clone());
        let mut state = Self::with_wasi(None, builder);
        state.observed.stdout = Some(stdout.clone());
        state
    }

    /// A state with no registrations served by `builder`, reading `clock` when the test
    /// supplies one.
    fn with_wasi(clock: Option<Clock>, mut builder: WasiCtxBuilder) -> Self {
        if let Some(clock) = &clock {
            builder.wall_clock(clock.clone());
        }
        Self {
            wasi: builder.build(),
            clock,
            table: ResourceTable::new(),
            abort_notice: None,
            observed: Observed::default(),
            guest: None,
            schema_origin: None,
        }
    }

    /// Cancels a retained resource by its table key.
    pub fn abort(&mut self, flag: u32) -> wasmtime::Result<()> {
        self.table
            .get_mut(&Resource::<Flag>::new_borrow(flag))?
            .0
            .store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(notice) = self.abort_notice.take() {
            let _ = notice.send(());
        }
        Ok(())
    }

    /// Adds a resource to the table and counts it as lent to the extension.
    pub(super) fn push<T: Send + 'static>(&mut self, value: T) -> wasmtime::Result<Resource<T>> {
        self.observed.lend();
        Ok(self.table.push(value)?)
    }

    /// Removes a resource the component dropped.
    pub(super) fn release<T: 'static>(&mut self, this: Resource<T>) -> wasmtime::Result<()> {
        self.table.delete(this)?;
        self.observed.reclaim();
        Ok(())
    }
}

impl session::Host for State {}

impl host::HostCallback for State {
    fn schema_invoke(
        &mut self,
        _: Resource<Identity>,
        _: host::SchemaOperation,
        _: Resource<JsonNode>,
        _: String,
    ) -> wasmtime::Result<Resource<Route>> {
        self.routed(Err("foreign schemas use the production runtime".into()))
    }

    fn new(&mut self) -> wasmtime::Result<Resource<Identity>> {
        let identity = self.observed.next_identity();
        Ok(self.table.push(Identity(identity, None))?)
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

impl host::HostBashOutput for State {
    fn on_data(
        &mut self,
        this: Resource<super::shell_host::Receiver>,
        bytes: Vec<u8>,
    ) -> wasmtime::Result<Result<(), String>> {
        let id = self.table.get(&this)?.0;
        Ok(self.observed.shell.outputs[id].data(&bytes))
    }
    fn drop(&mut self, this: Resource<super::shell_host::Receiver>) -> wasmtime::Result<()> {
        let id = self.table.get(&this)?.0;
        self.observed.shell.outputs[id].drops += 1;
        self.release(this)
    }
}

impl host::HostToolUpdate for State {
    fn update(
        &mut self,
        _this: Resource<Update>,
        partial: String,
    ) -> wasmtime::Result<Resource<Route>> {
        let value: serde_json::Value = serde_json::from_str(&partial)?;
        let result = self.observed.update(&partial, &value["details"]);
        self.routed(result)
    }
    fn drop(&mut self, this: Resource<Update>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

impl host::HostAbortSignal for State {
    fn retain(&mut self, this: Resource<Flag>) -> wasmtime::Result<Resource<Flag>> {
        let signal = self.table.get(&this)?.0.clone();
        self.push(Flag(signal))
    }

    fn abort(&mut self, this: Resource<Flag>) -> wasmtime::Result<Resource<Route>> {
        let result = self.abort(this.rep()).map_err(|error| error.to_string());
        self.routed(result)
    }

    fn aborted(&mut self, this: Resource<Flag>) -> wasmtime::Result<bool> {
        Ok(self
            .table
            .get(&this)?
            .0
            .load(std::sync::atomic::Ordering::SeqCst))
    }

    fn drop(&mut self, this: Resource<Flag>) -> wasmtime::Result<()> {
        *self
            .observed
            .shell
            .signal_drops
            .entry(this.rep() as usize)
            .or_default() += 1;
        self.release(this)
    }
}

/// Methods every session-bound context resource shares.
macro_rules! context_methods {
    ($kind:ident) => {
        fn ui(
            &mut self,
            this: Resource<$kind>,
        ) -> wasmtime::Result<Result<Resource<super::ui_host::Ui>, String>> {
            if let Err(e) = self.observed.catalog.check() {
                return Ok(Err(e));
            }
            let cwd = &self.table.get(&this)?.0.cwd;
            let key = self.observed.ui.acquire(cwd);
            Ok(Ok(self.push(super::ui_host::Ui(key))?))
        }
        fn has_ui(&mut self, this: Resource<$kind>) -> wasmtime::Result<Result<bool, String>> {
            let _ = &this;
            if let Err(e) = self.observed.catalog.check() {
                return Ok(Err(e));
            }
            Ok(Ok(self.observed.catalog.data.has_ui))
        }
        fn model_registry(
            &mut self,
            this: Resource<$kind>,
        ) -> wasmtime::Result<Result<Resource<super::reader_host::Reader>, String>> {
            let _ = &this;
            if let Err(e) = self.observed.catalog.check() {
                return Ok(Err(e));
            }
            let data = self.observed.catalog.registry.clone();
            Ok(Ok(self.push(data)?))
        }
        fn model(
            &mut self,
            this: Resource<$kind>,
        ) -> wasmtime::Result<Result<Option<String>, String>> {
            let _ = &this;
            if let Err(e) = self.observed.catalog.check() {
                return Ok(Err(e));
            }
            Ok(self
                .observed
                .catalog
                .selected_model(&self.table.get(&this)?.1))
        }
        fn is_idle(&mut self, this: Resource<$kind>) -> wasmtime::Result<Result<bool, String>> {
            let _ = &this;
            if let Err(e) = self.observed.catalog.check() {
                return Ok(Err(e));
            }
            Ok(Ok(self.observed.catalog.data.idle))
        }
        fn signal(
            &mut self,
            this: Resource<$kind>,
        ) -> wasmtime::Result<Result<Option<Resource<Flag>>, String>> {
            let _ = &this;
            if let Err(e) = self.observed.catalog.check() {
                return Ok(Err(e));
            }
            let flag = self.observed.catalog.flag.clone();
            Ok(Ok(flag.map(|flag| self.push(Flag(flag))).transpose()?))
        }
        fn abort(&mut self, this: Resource<$kind>) -> wasmtime::Result<Resource<Route>> {
            let _ = &this;
            if let Err(e) = self.observed.catalog.check() {
                return self.routed(Err(e));
            }
            self.observed.log("context abort");
            self.routed(Ok(()))
        }
        fn has_pending_messages(
            &mut self,
            this: Resource<$kind>,
        ) -> wasmtime::Result<Result<bool, String>> {
            let _ = &this;
            if let Err(e) = self.observed.catalog.check() {
                return Ok(Err(e));
            }
            Ok(Ok(self.observed.catalog.data.queued))
        }
        fn shutdown(&mut self, this: Resource<$kind>) -> wasmtime::Result<Resource<Route>> {
            let _ = &this;
            if let Err(e) = self.observed.catalog.check() {
                return self.routed(Err(e));
            }
            self.observed.log("context shutdown");
            self.routed(Ok(()))
        }
        fn get_context_usage(
            &mut self,
            this: Resource<$kind>,
        ) -> wasmtime::Result<Result<Option<host::ContextUsage>, String>> {
            let _ = &this;
            if let Err(e) = self.observed.catalog.check() {
                return Ok(Err(e));
            }
            Ok(Ok(self.observed.catalog.data.usage.as_ref().map(|u| {
                host::ContextUsage {
                    tokens: u.tokens,
                    context_window: u.context_window,
                    percent: u.percent,
                }
            })))
        }
        fn get_system_prompt(
            &mut self,
            this: Resource<$kind>,
        ) -> wasmtime::Result<Result<String, String>> {
            let _ = &this;
            if let Err(e) = self.observed.catalog.check() {
                return Ok(Err(e));
            }
            Ok(Ok(self.observed.catalog.data.prompt.clone()))
        }

        fn compact(
            &mut self,
            this: Resource<$kind>,
            instructions: Option<host::CompactionData>,
            complete: Option<Resource<Identity>>,
            error: Option<Resource<Identity>>,
        ) -> wasmtime::Result<Resource<Route>> {
            if let Err(error) = self.observed.actions.check() {
                return self.routed(Err(error));
            }
            let cwd = match self.table.get(&this)?.0.cwd() {
                Ok(cwd) => cwd,
                Err(error) => return self.routed(Err(error)),
            };
            self.observed.log(format!("compact binding {cwd}"));
            let result = super::action_host::compact(
                &mut self.observed,
                instructions
                    .as_ref()
                    .and_then(|data| data.custom_instructions.as_deref()),
                complete.map(|c| c.rep()),
                error.map(|c| c.rep()),
                instructions.is_some(),
            );
            self.routed(result)
        }
        fn cwd(&mut self, this: Resource<$kind>) -> wasmtime::Result<Result<String, String>> {
            Ok(self.table.get(&this)?.0.cwd())
        }

        fn session_manager(
            &mut self,
            this: Resource<$kind>,
        ) -> wasmtime::Result<Result<Resource<super::reader_host::Reader>, String>> {
            match self.session_reader(&this)? {
                Ok(reader) => Ok(Ok(self.push(reader)?)),
                Err(error) => Ok(Err(error)),
            }
        }

        fn drop(&mut self, this: Resource<$kind>) -> wasmtime::Result<()> {
            self.release(this)
        }
    };
}

/// Obtain an ordinary reader through the current host decision.
trait BoundReader {
    fn reader(&self) -> Option<super::reader_host::Reader>;
}
impl BoundReader for Ordinary {
    fn reader(&self) -> Option<super::reader_host::Reader> {
        None
    }
}
impl BoundReader for Command {
    fn reader(&self) -> Option<super::reader_host::Reader> {
        self.2.clone()
    }
}
impl State {
    /// Use a captured setup reader rather than the latest host context.
    fn session_reader<T: BoundReader + 'static>(
        &self,
        this: &Resource<T>,
    ) -> wasmtime::Result<Result<super::reader_host::Reader, String>> {
        Ok(self
            .table
            .get(this)?
            .reader()
            .map_or_else(|| self.observed.readers.acquire(), Ok))
    }
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
        self.push(Command(
            Session::new(&cwd),
            self.observed.catalog.model.clone(),
            self.table.get(&this)?.1.clone(),
        ))
    }

    fn drop(&mut self, this: Resource<Replaced>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

impl host::HostSubscription for State {
    fn unsubscribe(
        &mut self,
        this: Resource<super::bus_host::Subscription>,
    ) -> wasmtime::Result<Result<(), String>> {
        Ok(self.observed.bus.unsubscribe(self.table.get(&this)?))
    }
    fn drop(&mut self, this: Resource<super::bus_host::Subscription>) -> wasmtime::Result<()> {
        self.table.delete(this)?;
        Ok(())
    }
}
impl host::HostCallbackEmitter for State {
    fn drop(&mut self, this: Resource<super::bus_host::Emitter>) -> wasmtime::Result<()> {
        self.table.delete(this)?;
        Ok(())
    }
}
/// A host operation's nested calls into this instance, in order, before its own result.
pub struct Route {
    /// Nested calls not handed out yet, each with what its result settles.
    pub(super) steps: std::collections::VecDeque<(host::LocalCall, super::ui_host::Nested)>,
    /// What the nested call the guest is running settles.
    pub(super) running: Option<super::ui_host::Nested>,
    /// The operation's own result.
    pub(super) result: Option<Result<host::RouteValue, String>>,
    /// A rendering's pieces, which its nested results complete into its rows.
    pub(super) rows: Option<super::widget_component::Rows>,
}

/// A deferred nested call; the test host never makes one.
pub struct Settlement;

impl host::HostLocalSettlement for State {
    fn finish(&mut self, _: Resource<Settlement>, _: host::LocalResult) -> wasmtime::Result<()> {
        Ok(())
    }
    fn drop(&mut self, this: Resource<Settlement>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

impl State {
    /// Returns a routed import's result through its completed route.
    pub(super) fn routed(
        &mut self,
        result: Result<(), String>,
    ) -> wasmtime::Result<Resource<Route>> {
        self.routed_value(result.map(|()| host::RouteValue::Unit))
    }
    /// Preserves a non-unit routed result for a controlled resource operation.
    pub(super) fn routed_value(
        &mut self,
        result: Result<host::RouteValue, String>,
    ) -> wasmtime::Result<Resource<Route>> {
        self.push(Route {
            steps: std::collections::VecDeque::new(),
            running: None,
            rows: None,
            result: Some(result),
        })
    }
}

impl host::HostImportRoute for State {
    fn schema_result(
        &mut self,
        _: Resource<Route>,
    ) -> wasmtime::Result<Result<String, host::DiagnosticErrorInfo>> {
        Ok(Err(host::DiagnosticErrorInfo {
            name: None,
            message: "foreign schemas use the production runtime".into(),
            stack: None,
            code: None,
        }))
    }

    fn next(&mut self, this: Resource<Route>) -> wasmtime::Result<host::RouteStep> {
        let route = self.table.get_mut(&this)?;
        if let Some((call, nested)) = route.steps.pop_front() {
            route.running = Some(nested);
            return Ok(host::RouteStep::Invoke(call));
        }
        if let Some(rows) = route.rows.as_mut() {
            return Ok(host::RouteStep::Done(
                rows.rows().map(host::RouteValue::Lines),
            ));
        }
        Ok(host::RouteStep::Done(
            route.result.take().unwrap_or(Ok(host::RouteValue::Unit)),
        ))
    }
    fn resume(&mut self, this: Resource<Route>, value: host::LocalResult) -> wasmtime::Result<()> {
        let route = self.table.get_mut(&this)?;
        let nested = route.running.take();
        if let Some(rows) = route.rows.as_mut() {
            rows.results.push_back(value);
            return Ok(());
        }
        if let Some(super::ui_host::Nested::Shortcut(key, data)) = &nested
            && let host::LocalResult::Widget(Ok(host::WidgetAnswer::Consumed(false))) = &value
        {
            let continued = super::widget_editors::proceed(&mut self.observed, *key, data);
            match continued {
                Ok(calls) => {
                    let steps = self.steps(*key, calls)?;
                    self.table.get_mut(&this)?.steps.extend(steps);
                }
                Err(error) => self.table.get_mut(&this)?.result = Some(Err(error)),
            }
        }
        let failure = match nested {
            Some(nested) => self.settle_nested(nested, value)?,
            None => None,
        };
        if let Some(error) = failure {
            self.table.get_mut(&this)?.result = Some(Err(error));
        }
        Ok(())
    }
    fn defer(&mut self, _: Resource<Route>) -> wasmtime::Result<Resource<Settlement>> {
        self.push(Settlement)
    }
    fn drop(&mut self, this: Resource<Route>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

impl host::Host for State {
    fn create_local_bash_operations(
        &mut self,
        _: Option<String>,
    ) -> wasmtime::Result<Result<Resource<LocalBashOperations>, String>> {
        wasmtime::bail!("local shell operations require the production host")
    }
    fn with_file_mutation_queue(
        &mut self,
        _: String,
    ) -> wasmtime::Result<Result<Resource<FileMutation>, String>> {
        wasmtime::bail!("file mutation queues require the production host")
    }

    fn new_overlay_options(
        &mut self,
        encoded: String,
    ) -> wasmtime::Result<Result<Resource<super::overlay_host::Options>, String>> {
        super::overlay_host::create(self, &encoded)
    }
    fn new_text(
        &mut self,
        text: String,
        padding_x: u32,
        padding_y: u32,
        background: Option<host::StyleSource>,
    ) -> wasmtime::Result<Result<Resource<super::widget_component::WidgetResource>, String>> {
        let paint = self.paints(background.as_ref())?;
        self.build_widget(super::widget_host::Build {
            kind: "text",
            text,
            padding: (padding_x, padding_y),
            paint,
            ..Default::default()
        })
    }
    fn new_box(
        &mut self,
        padding_x: u32,
        padding_y: u32,
        background: Option<host::StyleSource>,
    ) -> wasmtime::Result<Result<Resource<super::widget_component::WidgetResource>, String>> {
        let paint = self.paints(background.as_ref())?;
        self.build_widget(super::widget_host::Build {
            kind: "box",
            padding: (padding_x, padding_y),
            paint,
            ..Default::default()
        })
    }
    fn new_container(
        &mut self,
    ) -> wasmtime::Result<Result<Resource<super::widget_component::WidgetResource>, String>> {
        self.build_widget(super::widget_host::Build {
            kind: "container",
            ..Default::default()
        })
    }
    fn new_spacer(
        &mut self,
        lines: u32,
    ) -> wasmtime::Result<Result<Resource<super::widget_component::WidgetResource>, String>> {
        self.build_widget(super::widget_host::Build {
            kind: "spacer",
            lines,
            ..Default::default()
        })
    }
    fn new_markdown(
        &mut self,
        text: String,
        padding_x: u32,
        padding_y: u32,
        theme: host::MarkdownStyles,
        style: Option<host::DefaultTextStyle>,
    ) -> wasmtime::Result<Result<Resource<super::widget_component::WidgetResource>, String>> {
        self.build_markdown(text, (padding_x, padding_y), &theme, style.as_ref())
    }
    fn new_dynamic_border(
        &mut self,
        color: Option<host::StyleSource>,
    ) -> wasmtime::Result<Result<Resource<super::widget_component::WidgetResource>, String>> {
        let paint = self.paints(color.as_ref())?;
        self.build_widget(super::widget_host::Build {
            kind: "border",
            paint,
            ..Default::default()
        })
    }
    fn global_theme(
        &mut self,
    ) -> wasmtime::Result<Result<Resource<super::renderer_host::Theme>, String>> {
        Ok(Ok(self
            .table
            .push(super::renderer_host::Theme("dark".into()))?))
    }
    fn get_markdown_theme(&mut self) -> wasmtime::Result<Result<host::MarkdownTheme, String>> {
        self.markdown_theme().map(Ok)
    }
    fn new_select_list(
        &mut self,
        items: Vec<host::SelectItem>,
        max_visible: u32,
        theme: host::SelectListStyles,
        layout: host::SelectLayout,
    ) -> wasmtime::Result<Result<Resource<super::widget_component::WidgetResource>, String>> {
        self.build_select_list(items, max_visible, &theme, &layout)
    }
    fn new_settings_list(
        &mut self,
        items: Vec<host::SettingItem>,
        max_visible: u32,
        theme: host::SettingsListStyles,
        on_change: Resource<Identity>,
        on_cancel: Resource<Identity>,
        enable_search: bool,
    ) -> wasmtime::Result<Result<Resource<super::widget_component::WidgetResource>, String>> {
        let callbacks = (self.table.get(&on_change)?.0, self.table.get(&on_cancel)?.0);
        self.build_settings_list((items, max_visible), &theme, callbacks, enable_search)
    }
    fn new_editor(
        &mut self,
        _: Resource<super::ui_host::Tui>,
        theme: host::EditorStyles,
        options: host::EditorOptions,
    ) -> wasmtime::Result<Result<Resource<super::widget_component::WidgetResource>, String>> {
        self.build_editor(&theme, None, options)
    }
    fn new_custom_editor(
        &mut self,
        _: Resource<super::ui_host::Tui>,
        theme: host::EditorStyles,
        keybindings: Resource<super::widget_component::KeybindingsResource>,
        options: host::EditorOptions,
    ) -> wasmtime::Result<Result<Resource<super::widget_component::WidgetResource>, String>> {
        let keys = self.table.get(&keybindings)?.0;
        self.build_editor(&theme, Some(keys), options)
    }
    fn new_bordered_loader(
        &mut self,
        _: Resource<super::ui_host::Tui>,
        theme: Resource<super::renderer_host::Theme>,
        message: String,
        cancellable: Option<bool>,
    ) -> wasmtime::Result<Result<Resource<super::widget_component::WidgetResource>, String>> {
        self.build_loader(&theme, message, cancellable)
    }
    fn get_select_list_theme(&mut self) -> wasmtime::Result<Result<host::SelectListTheme, String>> {
        self.observed.log("getSelectListTheme");
        let mut role =
            |name: &str| self.push(super::widget_component::StyleResource(name.to_owned()));
        Ok(Ok(host::SelectListTheme {
            selected_prefix: role("selectedPrefix")?,
            selected_text: role("selectedText")?,
            description: role("description")?,
            scroll_info: role("scrollInfo")?,
            no_match: role("noMatch")?,
        }))
    }
    fn new_keybindings_manager(
        &mut self,
        user_bindings: Vec<(String, Option<host::KeybindingKeys>)>,
        config_path: Option<String>,
    ) -> wasmtime::Result<Result<Resource<super::widget_component::KeybindingsResource>, String>>
    {
        let user = super::widget_component::config(user_bindings);
        self.keep_keys(None, user, config_path)
    }
    fn create_keybindings_manager(
        &mut self,
        agent_dir: Option<String>,
    ) -> wasmtime::Result<Result<Resource<super::widget_component::KeybindingsResource>, String>>
    {
        let path = format!(
            "{}/keybindings.json",
            agent_dir.as_deref().unwrap_or("/agent")
        );
        self.keep_keys(None, Vec::new(), Some(path))
    }
    fn new_keybindings_with_definitions(
        &mut self,
        definitions: Vec<(String, host::KeybindingDefinition)>,
        user_bindings: Vec<(String, Option<host::KeybindingKeys>)>,
    ) -> wasmtime::Result<Result<Resource<super::widget_component::KeybindingsResource>, String>>
    {
        let definitions = super::widget_component::definitions(definitions);
        let user = super::widget_component::config(user_bindings);
        self.keep_keys(Some(definitions), user, None)
    }
    fn get_keybindings(
        &mut self,
    ) -> wasmtime::Result<Result<Resource<super::widget_component::KeybindingsResource>, String>>
    {
        self.observed.log("getKeybindings");
        Ok(Ok(
            self.push(super::widget_component::KeybindingsResource(0))?
        ))
    }
    fn key_text(&mut self, keybinding: String) -> wasmtime::Result<Result<String, String>> {
        Ok(Ok(super::widget_keys::hint(
            &mut self.observed,
            Some(&keybinding),
            None,
            None,
        )))
    }
    fn key_hint(
        &mut self,
        keybinding: String,
        description: String,
    ) -> wasmtime::Result<Result<String, String>> {
        let hint = super::widget_keys::hint(
            &mut self.observed,
            Some(&keybinding),
            None,
            Some(&description),
        );
        Ok(Ok(hint))
    }
    fn raw_key_hint(
        &mut self,
        key: String,
        description: String,
    ) -> wasmtime::Result<Result<String, String>> {
        let hint =
            super::widget_keys::hint(&mut self.observed, None, Some(&key), Some(&description));
        Ok(Ok(hint))
    }
    fn get_editor_theme(&mut self) -> wasmtime::Result<Result<host::EditorTheme, String>> {
        self.editor_theme().map(Ok)
    }
    fn get_settings_list_theme(
        &mut self,
    ) -> wasmtime::Result<Result<host::SettingsListTheme, String>> {
        self.observed.log("getSettingsListTheme");
        Ok(Ok(host::SettingsListTheme {
            label: self.push(super::widget_component::RowStyleResource(
                "label".to_owned(),
            ))?,
            value: self.push(super::widget_component::RowStyleResource(
                "value".to_owned(),
            ))?,
            description: self.push(super::widget_component::StyleResource(
                "description".to_owned(),
            ))?,
            cursor: "→ ".to_owned(),
            hint: self.push(super::widget_component::StyleResource("hint".to_owned()))?,
        }))
    }

    fn watch(
        &mut self,
        path: String,
        listener: Resource<Identity>,
    ) -> wasmtime::Result<Result<Resource<super::native_watch_host::Watcher>, String>> {
        match self
            .observed
            .native_watch
            .component_watch(path, listener.rep())
        {
            Ok(watcher) => Ok(Ok(self.push(watcher)?)),
            Err(error) => Ok(Err(error)),
        }
    }
    fn kill(&mut self, pid: i32, signal: Option<String>) -> wasmtime::Result<Result<(), String>> {
        Ok(self.observed.native_io.kill(pid, signal))
    }
    fn model_stream(
        &mut self,
        _: bool,
        _: String,
        _: host::ModelControls,
    ) -> wasmtime::Result<Result<Resource<super::stream_host::Assistant>, String>> {
        Ok(Err(super::stream_host::PRODUCTION_HOST.into()))
    }

    fn new_event_stream(
        &mut self,
        _: Resource<Identity>,
        _: Resource<Identity>,
    ) -> wasmtime::Result<Result<Resource<super::stream_host::Stream>, String>> {
        Ok(Err(super::stream_host::PRODUCTION_HOST.into()))
    }

    fn create_grep_tool(
        &mut self,
        _: String,
        _: Option<Resource<Identity>>,
    ) -> wasmtime::Result<Result<Resource<BuiltinTool>, String>> {
        wasmtime::bail!("built-in tools require the production host")
    }
    fn create_ls_tool(
        &mut self,
        _: String,
        _: Option<Resource<Identity>>,
    ) -> wasmtime::Result<Result<Resource<BuiltinTool>, String>> {
        wasmtime::bail!("built-in tools require the production host")
    }
    fn create_find_tool(
        &mut self,
        _: String,
        _: Option<Resource<Identity>>,
    ) -> wasmtime::Result<Result<Resource<BuiltinTool>, String>> {
        wasmtime::bail!("built-in tools require the production host")
    }
    fn create_write_tool(
        &mut self,
        _: String,
        _: Option<Resource<Identity>>,
    ) -> wasmtime::Result<Result<Resource<BuiltinTool>, String>> {
        wasmtime::bail!("built-in tools require the production host")
    }
    fn create_edit_tool(
        &mut self,
        _: String,
        _: Option<Resource<Identity>>,
    ) -> wasmtime::Result<Result<Resource<BuiltinTool>, String>> {
        wasmtime::bail!("built-in tools require the production host")
    }
    fn create_bash_tool(
        &mut self,
        _: String,
        _: host::BashToolControls,
        _: Option<Resource<Identity>>,
        _: Option<Resource<Identity>>,
    ) -> wasmtime::Result<Result<Resource<BuiltinTool>, String>> {
        wasmtime::bail!("built-in tools require the production host")
    }
    fn create_tool_preset(
        &mut self,
        _: host::ToolPreset,
        _: String,
        _: host::ToolFactoryOptions,
    ) -> wasmtime::Result<Result<Vec<(host::ToolName, Resource<BuiltinTool>)>, String>> {
        wasmtime::bail!("built-in tools require the production host")
    }
    fn create_tool(
        &mut self,
        _: String,
        _: String,
        _: host::ToolFactoryOptions,
    ) -> wasmtime::Result<Result<Resource<BuiltinTool>, String>> {
        wasmtime::bail!("built-in tools require the production host")
    }
    fn create_read_tool(
        &mut self,
        _: String,
        _: Option<bool>,
        _: Option<Resource<Identity>>,
    ) -> wasmtime::Result<Result<Resource<BuiltinTool>, String>> {
        wasmtime::bail!("built-in tools require the production host")
    }
    fn native_format_size(&mut self, _bytes: f64) -> wasmtime::Result<Result<String, String>> {
        Ok(self
            .observed
            .actions
            .record_for("format_size")
            .and_then(|value| serde_json::from_str(&value).map_err(|error| error.to_string())))
    }
    fn native_frontmatter(
        &mut self,
        content: String,
    ) -> wasmtime::Result<Result<host::FrontmatterDocument, String>> {
        if content != "---\nx: .nan\n---\nbody" {
            return Ok(Err("unknown controlled metadata fixture".into()));
        }
        Ok(Ok(host::FrontmatterDocument {
            values: vec![
                host::FrontmatterToken::Mapping(vec!["x".into()]),
                host::FrontmatterToken::Number(f64::NAN),
            ],
            body: "body".into(),
        }))
    }
    fn native_yaml(
        &mut self,
        _content: String,
    ) -> wasmtime::Result<Result<Vec<host::FrontmatterToken>, String>> {
        Ok(Err("raw YAML requires the production helper host".into()))
    }
    fn native_helper(
        &mut self,
        operation: String,
        data: String,
    ) -> wasmtime::Result<Result<String, String>> {
        self.observed
            .helper_calls
            .push((operation.clone(), data.clone()));
        Ok(self.observed.actions.helper(&operation, &data))
    }
    fn callback_emit(
        &mut self,
        emitter: Resource<super::bus_host::Emitter>,
        channel: String,
        data: String,
    ) -> wasmtime::Result<Result<(u32, Vec<Option<u32>>), String>> {
        self.snapshot_listeners(&emitter, &channel, &data)
    }
    fn callback_begin_local(
        &mut self,
        emitter: Resource<super::bus_host::Emitter>,
        snapshot: u32,
        index: u32,
    ) -> wasmtime::Result<Result<Resource<super::bus_host::Emitter>, String>> {
        self.begin_local_listener(&emitter, snapshot, index)
    }
    fn callback_end_local(
        &mut self,
        _: Resource<super::bus_host::Emitter>,
        error: Option<String>,
    ) -> wasmtime::Result<Resource<Route>> {
        if let Some(error) = error {
            self.observed.log(format!("listener error: {error}"));
        }
        self.routed(Ok(()))
    }
    fn callback_segment(
        &mut self,
        _: Resource<super::bus_host::Emitter>,
        _: u32,
        start: u32,
        end: u32,
    ) -> wasmtime::Result<Resource<Route>> {
        if start == end {
            return self.routed(Ok(()));
        }
        self.routed(Err("author fixture has only guest-local listeners".into()))
    }
    fn callback_end_snapshot(
        &mut self,
        _: Resource<super::bus_host::Emitter>,
        snapshot: u32,
    ) -> wasmtime::Result<Result<(), String>> {
        Ok(self
            .observed
            .bus
            .snapshots
            .get_mut(snapshot as usize)
            .and_then(Option::take)
            .map(drop)
            .ok_or_else(|| "snapshot ended".into()))
    }

    fn register_message_renderer(
        &mut self,
        custom_type: String,
        handler: Resource<Identity>,
    ) -> wasmtime::Result<Result<(), String>> {
        if let Some(error) = self.observed.registrations.refusal() {
            return Ok(Err(error));
        }
        self.observed
            .register(format!("renderer {custom_type}"), handler.rep());
        self.observed.log(format!(
            "renderer-register {}",
            serde_json::json!(custom_type)
        ));
        Ok(Ok(()))
    }
    fn send_message(
        &mut self,
        message: String,
        options: Option<String>,
    ) -> wasmtime::Result<Resource<Route>> {
        let result = super::action_host::action(
            &mut self.observed,
            "sendMessage",
            &serde_json::json!([
                super::action_host::document(Some(&message)),
                super::action_host::document(options.as_deref())
            ]),
        );
        self.routed(result)
    }
    fn send_user_message(
        &mut self,
        content: String,
        options: Option<String>,
    ) -> wasmtime::Result<Resource<Route>> {
        let result = super::action_host::action(
            &mut self.observed,
            "sendUserMessage",
            &serde_json::json!([
                super::action_host::document(Some(&content)),
                super::action_host::document(options.as_deref())
            ]),
        );
        self.routed(result)
    }
    fn set_session_name(&mut self, name: String) -> wasmtime::Result<Resource<Route>> {
        let result = super::action_host::action(
            &mut self.observed,
            "setSessionName",
            &serde_json::json!([name]),
        );
        self.routed(result)
    }
    fn set_label(
        &mut self,
        entry_id: String,
        label: Option<String>,
    ) -> wasmtime::Result<Resource<Route>> {
        let result = super::action_host::action(
            &mut self.observed,
            "setLabel",
            &serde_json::json!([
                entry_id,
                label.map_or(
                    serde_json::json!({"present":false}),
                    |v| serde_json::json!({"present":true,"value":v})
                )
            ]),
        );
        self.routed(result)
    }
    fn set_active_tools(&mut self, tool_names: Vec<String>) -> wasmtime::Result<Resource<Route>> {
        let result = super::action_host::action(
            &mut self.observed,
            "setActiveTools",
            &serde_json::json!([tool_names]),
        );
        self.routed(result)
    }
    fn set_thinking_level(&mut self, level: String) -> wasmtime::Result<Resource<Route>> {
        let result = super::action_host::action(
            &mut self.observed,
            "setThinkingLevel",
            &serde_json::json!([serde_json::from_str::<serde_json::Value>(&level).unwrap()]),
        );
        self.routed(result)
    }
    fn get_session_name(&mut self) -> wasmtime::Result<Result<Option<String>, String>> {
        Ok(self
            .observed
            .actions
            .check()
            .map(|()| self.observed.actions.value.as_str().map(str::to_owned)))
    }
    fn get_active_tools(&mut self) -> wasmtime::Result<Result<Vec<String>, String>> {
        Ok(self.observed.actions.check().and_then(|()| {
            serde_json::from_value(
                self.observed
                    .actions
                    .value
                    .get("getActiveTools")
                    .unwrap_or(&self.observed.actions.value)
                    .clone(),
            )
            .map_err(|e| e.to_string())
        }))
    }
    fn get_commands(&mut self) -> wasmtime::Result<Result<String, String>> {
        Ok(self.observed.actions.record())
    }
    fn get_thinking_level(&mut self) -> wasmtime::Result<Result<String, String>> {
        Ok(self.observed.actions.record_for("getThinkingLevel"))
    }

    fn event_on(
        &mut self,
        channel: String,
        handler: Resource<Identity>,
    ) -> wasmtime::Result<Result<Resource<super::bus_host::Subscription>, String>> {
        match self.observed.bus.on(&channel, handler.rep()) {
            Ok(subscription) => {
                self.observed
                    .on(&format!("bus {}", handler.rep()), handler.rep())
                    .map_err(wasmtime::Error::msg)?;
                Ok(Ok(self.table.push(subscription)?))
            }
            Err(error) => Ok(Err(error)),
        }
    }
    fn new_signal(&mut self) -> wasmtime::Result<Result<Resource<Flag>, String>> {
        Ok(Ok(self.push(Flag(Arc::default()))?))
    }

    fn signal_subscribe(
        &mut self,
        signal: Resource<Flag>,
        listener: Resource<Identity>,
    ) -> wasmtime::Result<Result<Resource<super::bus_host::Subscription>, String>> {
        let key = Arc::as_ptr(&self.table.get(&signal)?.0).addr();
        match self
            .observed
            .bus
            .on(&format!("abort {key}"), listener.rep())
        {
            Ok(subscription) => Ok(Ok(self.table.push(subscription)?)),
            Err(error) => Ok(Err(error)),
        }
    }
    fn start_event_tail(
        &mut self,
        _emitter: Resource<super::bus_host::Emitter>,
        tail: Resource<Identity>,
    ) -> wasmtime::Result<Result<(), String>> {
        Ok(self.observed.bus.tail(tail.rep()))
    }

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
        description: Option<String>,
        completions: Option<Resource<Identity>>,
        handler: Resource<Identity>,
    ) -> wasmtime::Result<Result<(), String>> {
        if let Some(error) = self.observed.registrations.refusal() {
            return Ok(Err(error));
        }
        self.observed.register_command(&name, handler.rep());
        self.observed.registrations.commands.push((
            name.clone(),
            description,
            completions.is_some(),
        ));
        if let Some(completions) = completions {
            self.observed
                .register(format!("completion {name}"), completions.rep());
        }
        Ok(Ok(()))
    }
    fn register_shortcut(
        &mut self,
        shortcut: String,
        description: Option<String>,
        handler: Resource<Identity>,
    ) -> wasmtime::Result<Result<(), String>> {
        if let Some(error) = self.observed.registrations.refusal() {
            return Ok(Err(error));
        }
        self.observed
            .registrations
            .shortcuts
            .push((shortcut.clone(), description));
        self.observed
            .register(format!("shortcut {shortcut}"), handler.rep());
        Ok(Ok(()))
    }
    fn contribute_load_time(
        &mut self,
        contributions: String,
    ) -> wasmtime::Result<Result<(), String>> {
        self.observed.log(format!("load-time {contributions}"));
        Ok(Ok(()))
    }
    fn register_flag(
        &mut self,
        name: String,
        options: host::FlagOptions,
    ) -> wasmtime::Result<Result<(), String>> {
        if let Some(error) = self.observed.registrations.refusal() {
            return Ok(Err(error));
        }
        let options = crate::FlagOptions {
            description: options.description,
            type_: match options.type_ {
                host::FlagType::Boolean => crate::FlagType::Boolean,
                host::FlagType::String => crate::FlagType::String,
            },
            default: options.default.map(|v| match v {
                host::FlagValue::Boolean(v) => crate::FlagValue::Boolean(v),
                host::FlagValue::String(v) => crate::FlagValue::String(v),
            }),
        };
        self.observed
            .registrations
            .flags
            .push((name, super::registration_host::options(&options)));
        Ok(Ok(()))
    }
    fn get_flag(
        &mut self,
        name: String,
    ) -> wasmtime::Result<Result<Option<host::FlagValue>, String>> {
        Ok(self.observed.registrations.value(&name).map(|v| {
            v.map(|v| match v {
                crate::FlagValue::Boolean(v) => host::FlagValue::Boolean(v),
                crate::FlagValue::String(v) => host::FlagValue::String(v),
            })
        }))
    }

    fn auth_storage_from_backend(
        &mut self,
        backend: Resource<Identity>,
    ) -> wasmtime::Result<Resource<Route>> {
        let reader = match self.observed.readers.acquire() {
            Ok(reader) => reader.fresh(),
            Err(error) => return self.routed(Err(error)),
        };
        if let Err(error) = reader.unit("auth_storage_from_backend", &[]) {
            return self.routed(Err(error));
        }
        reader.1.lock().unwrap().backend = Some(self.table.get(&backend)?.0);
        self.observed
            .register("credential-backend".into(), backend.rep());
        let owner = self.push(reader.clone())?;
        self.credential_backend_route(&reader, Ok(host::RouteValue::Credentials(owner)))
    }
    fn create_auth_storage(
        &mut self,
        path: Option<String>,
    ) -> wasmtime::Result<Result<Resource<super::reader_host::Reader>, String>> {
        let result = self
            .observed
            .readers
            .acquire()
            .map(|reader| reader.fresh())
            .and_then(|reader| {
                reader.unit("create_auth_storage", &[serde_json::json!(path)])?;
                self.table.push(reader).map_err(|error| error.to_string())
            });
        Ok(result)
    }
    fn auth_storage_in_memory(
        &mut self,
        data: String,
    ) -> wasmtime::Result<Result<Resource<super::reader_host::Reader>, String>> {
        let result = self
            .observed
            .readers
            .acquire()
            .map(|reader| reader.fresh())
            .and_then(|reader| {
                reader.unit("auth_storage_in_memory", &[serde_json::json!(data)])?;
                self.table.push(reader).map_err(|error| error.to_string())
            });
        Ok(result)
    }
    fn create_model_registry(
        &mut self,
        auth: Resource<super::reader_host::Reader>,
        path: Option<String>,
    ) -> wasmtime::Result<Result<Resource<super::reader_host::Reader>, String>> {
        let reader = self.table.get(&auth)?.clone();
        Ok(reader
            .unit("create_model_registry", &[serde_json::json!(path)])
            .and_then(|()| self.table.push(reader).map_err(|error| error.to_string())))
    }
    fn in_memory_model_registry(
        &mut self,
        auth: Resource<super::reader_host::Reader>,
    ) -> wasmtime::Result<Result<Resource<super::reader_host::Reader>, String>> {
        let reader = self.table.get(&auth)?.clone();
        Ok(reader
            .unit("in_memory_model_registry", &[])
            .and_then(|()| self.table.push(reader).map_err(|error| error.to_string())))
    }
    fn register_provider(
        &mut self,
        name: String,
        config: String,
        _callbacks: (
            Option<Resource<super::host::Identity>>,
            Option<(Resource<super::host::Identity>, String, Option<bool>)>,
        ),
    ) -> wasmtime::Result<Resource<Route>> {
        self.observed
            .log(format!("provider register {name} {config}"));
        self.routed(Ok(()))
    }

    fn unregister_provider(&mut self, name: String) -> wasmtime::Result<Resource<Route>> {
        self.observed.log(format!("provider unregister {name}"));
        self.routed(Ok(()))
    }

    fn append_entry(
        &mut self,
        custom_type: String,
        data: Option<String>,
    ) -> wasmtime::Result<Resource<Route>> {
        if let Some(error) = self.observed.actions.entry_error.take() {
            return self.routed(Err(error));
        }
        if custom_type == "state Ω"
            && let Err(error) = self.observed.actions.check()
        {
            return self.routed(Err(error));
        }
        self.observed.entry(&custom_type, data.as_deref());
        self.routed(Ok(()))
    }
}

/// Selects the host state as the data every import runs against.
pub(super) struct Imports;

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
            Ok((guest, state.push(Replaced(Session::new(cwd), None))?))
        })?;
        guest
            .call_invoke_with_session(accessor, Resource::new_borrow(callback.rep()), replaced)
            .await
    }
}

impl Imports {
    /// Invoke continuation against the same reader supplied to setup.
    async fn continue_bound(
        accessor: &Accessor<State, Self>,
        bound: Option<super::reader_host::Reader>,
        with_session: Option<Resource<Identity>>,
    ) -> wasmtime::Result<Result<(), String>> {
        Ok(
            if let (Some(bound), Some(callback)) = (bound, with_session.as_ref()) {
                let (guest, replaced) = accessor.with(|mut a| -> wasmtime::Result<_> {
                    let state = a.get();
                    Ok((
                        state.guest.clone().unwrap(),
                        state.push(Replaced(Session::new("/replacement"), Some(bound)))?,
                    ))
                })?;
                guest
                    .call_invoke_with_session(accessor, borrow(callback.rep()), replaced)
                    .await?
            } else {
                Self::continue_with(accessor, with_session, "/replacement").await?
            },
        )
    }
}

impl host::HostCommandContextWithStore<State> for Imports {
    async fn fork(
        accessor: &Accessor<State, Self>,
        this: Resource<Command>,
        id: String,
        data: Option<String>,
        callback: Option<Resource<Identity>>,
    ) -> wasmtime::Result<Result<SessionChangeResult, String>> {
        super::replacement_host::binding(accessor, &this)?;
        super::replacement_host::command(accessor, "fork", &id, data.as_deref(), callback).await
    }
    async fn navigate_tree(
        accessor: &Accessor<State, Self>,
        this: Resource<Command>,
        id: String,
        data: Option<String>,
    ) -> wasmtime::Result<Result<SessionChangeResult, String>> {
        super::replacement_host::binding(accessor, &this)?;
        super::replacement_host::command(accessor, "navigateTree", &id, data.as_deref(), None).await
    }
    async fn switch_session(
        accessor: &Accessor<State, Self>,
        this: Resource<Command>,
        id: String,
        options: Option<host::SwitchSessionOptions>,
    ) -> wasmtime::Result<Result<SessionChangeResult, String>> {
        super::replacement_host::binding(accessor, &this)?;
        super::replacement_host::command(
            accessor,
            "switchSession",
            &id,
            options.as_ref().map(|_| "{}"),
            options.and_then(|options| options.with_session),
        )
        .await
    }
    async fn reload(
        accessor: &Accessor<State, Self>,
        this: Resource<Command>,
    ) -> wasmtime::Result<Result<(), String>> {
        super::replacement_host::binding(accessor, &this)?;
        Ok(
            super::replacement_host::command(accessor, "reload", "", None, None)
                .await?
                .map(|_| ()),
        )
    }

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
        data: Option<NewSessionCommandData>,
        setup: Option<Resource<Identity>>,
        with_session: Option<Resource<Identity>>,
    ) -> wasmtime::Result<Result<SessionChangeResult, String>> {
        let parent = data
            .as_ref()
            .and_then(|data| data.parent_session.as_deref())
            .unwrap_or("none");
        let hold = accessor.with(|mut a| {
            super::setup_host::command_start(
                &mut a.get().observed,
                data.as_ref()
                    .and_then(|data| data.parent_session.as_deref()),
                (setup.is_some(), with_session.is_some()),
                data.is_some(),
            )
        });
        pause(hold).await;
        if let Some(error) = accessor.with(|mut a| a.get().observed.setup.session_error.clone()) {
            return Ok(Err(error));
        }
        let line = format!("new-session start parent={parent}");
        accessor.with(|mut access| access.get().observed.log(line));
        if accessor.with(|mut access| access.get().observed.take_rejection()) {
            return Ok(Err("session rejected".to_owned()));
        }
        if accessor.with(|mut a| a.get().observed.setup.cancelled) {
            return Ok(Ok(SessionChangeResult { cancelled: true }));
        }
        if accessor.with(|mut a| a.get().observed.setup.unused) {
            return Ok(Ok(SessionChangeResult { cancelled: false }));
        }
        let bound = setup.as_ref().map(|_| super::setup_host::reader(parent));
        if let Some(callback) = setup {
            let (guest, writer) = accessor.with(|mut a| -> wasmtime::Result<_> {
                let state = a.get();
                Ok((
                    state.guest.clone().unwrap(),
                    state.push(bound.clone().unwrap())?,
                ))
            })?;
            if let Err(error) = guest
                .call_invoke_setup(accessor, borrow(callback.rep()), writer)
                .await?
            {
                return Ok(Err(error));
            }
        }
        let continuation = Self::continue_bound(accessor, bound, with_session).await?;
        if let Err(message) = continuation {
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

/// Delivers a compaction result or failure to `handler`, lending a fresh emitter for it.
pub async fn notify_compaction(
    harness: &mut Harness,
    handler: u32,
    payload: &str,
    complete: bool,
) -> Result<(), String> {
    let emitter = harness
        .store
        .data_mut()
        .table
        .push(super::bus_host::Emitter { key: 0 })
        .map_err(|error| error.to_string())?;
    let function = if complete {
        harness.exports.func_invoke_compaction_complete()
    } else {
        harness.exports.func_invoke_compaction_error()
    };
    let result = function
        .call_async(
            &mut harness.store,
            (
                borrow(handler),
                payload.to_owned(),
                Resource::new_borrow(emitter.rep()),
            ),
        )
        .await
        .map_err(|error| error.to_string())?
        .0;
    harness
        .store
        .data_mut()
        .table
        .delete(emitter)
        .map_err(|error| error.to_string())?;
    result
}

/// The instantiated component and the table keys of what it registered.
pub struct Harness {
    /// The store holding the host state and the instance.
    pub store: Store<State>,
    /// The guest exports.
    pub exports: Guest,
}

/// Shares one engine within a process and caches each component by its artifact path.
pub(super) fn compiled(path: &Path) -> wasmtime::Result<(Engine, Component)> {
    static ENGINE: OnceLock<Result<Engine, String>> = OnceLock::new();
    static COMPONENTS: LazyLock<Mutex<HashMap<PathBuf, Result<Component, String>>>> =
        LazyLock::new(Mutex::default);
    let engine = ENGINE
        .get_or_init(|| super::build::engine().map_err(|error| error.to_string()))
        .as_ref()
        .map_err(|message| wasmtime::format_err!("{message}"))?;
    COMPONENTS
        .lock()
        .unwrap()
        .entry(path.to_path_buf())
        .or_insert_with(|| Component::from_file(engine, path).map_err(|error| error.to_string()))
        .as_ref()
        .map(|component| (engine.clone(), component.clone()))
        .map_err(|message| wasmtime::format_err!("{message}"))
}

/// Build the existing test host's import wiring for one component instance.
fn component_linker(engine: &Engine) -> wasmtime::Result<Linker<State>> {
    let mut linker = Linker::new(engine);
    wasmtime_wasi::p2::add_to_linker_async(&mut linker)?;
    wasmtime_wasi::p3::add_to_linker(&mut linker)?;
    Extension::add_to_linker::<State, Imports>(&mut linker, |state| state)?;
    Ok(linker)
}

impl Harness {
    /// Formats the callback-table diagnostic using the resource's logical identity.
    pub fn callback_error(&self, key: u32) -> String {
        let id = self
            .store
            .data()
            .table
            .get(&Resource::<Identity>::new_borrow(key))
            .unwrap()
            .0;
        format!("no callback registered for identity {id}")
    }
    /// Instantiates the component and runs its factory.
    pub async fn start(path: &Path) -> wasmtime::Result<Self> {
        let (engine, component) = compiled(path)?;
        Self::from_compiled(&engine, &component).await
    }

    /// Instantiates one registered example and runs its factory.
    ///
    /// # Errors
    /// Returns the build, compilation or instantiation failure as text.
    pub fn example(name: &str) -> Result<Self, String> {
        let path = super::build::example_component("maestro-extensions-wasm", name)?;
        let (engine, component) =
            super::build::compile(&path).map_err(|error| error.to_string())?;
        super::block_on(Self::from_compiled(&engine, &component)).map_err(|error| error.to_string())
    }

    /// Instantiates a supplied compiled component and runs its factory.
    pub async fn from_compiled(engine: &Engine, component: &Component) -> wasmtime::Result<Self> {
        Self::start_on(engine, component, State::new()).await
    }

    /// Instantiates a component with explicitly supplied host replies and standard capabilities.
    pub async fn from_compiled_with_inputs(
        engine: &Engine,
        component: &Component,
        observed: Observed,
        wasi: WasiCtx,
    ) -> wasmtime::Result<Self> {
        let mut state = State::new();
        state.observed = observed;
        state.wasi = wasi;
        Self::start_on(engine, component, state).await
    }

    /// Instantiates a supplied component with controlled standard environment and streams.
    pub async fn from_compiled_with_wasi(
        engine: &Engine,
        component: &Component,
        wasi: WasiCtx,
    ) -> wasmtime::Result<Self> {
        let mut state = State::new();
        state.wasi = wasi;
        Self::start_on(engine, component, state).await
    }

    /// Instantiates a supplied compiled component whose standard library reads `clock`.
    pub async fn from_compiled_with_clock(
        engine: &Engine,
        component: &Component,
        clock: Clock,
    ) -> wasmtime::Result<Self> {
        Self::start_on(engine, component, State::with_clock(Some(clock))).await
    }

    /// Instantiates a supplied compiled component with the guest's standard output captured
    /// into a pipe of `size` bytes, which the harness also returns.
    pub async fn from_compiled_capturing(
        engine: &Engine,
        component: &Component,
        size: usize,
    ) -> wasmtime::Result<(Self, MemoryOutputPipe)> {
        let stdout = MemoryOutputPipe::new(size);
        let harness = Self::start_on(engine, component, State::capturing(&stdout)).await?;
        Ok((harness, stdout))
    }

    /// Instantiates a supplied compiled component whose host refuses every registration with
    /// `error`, and returns the harness beside the outcome of the factory's start.
    pub async fn from_compiled_refusing(
        engine: &Engine,
        component: &Component,
        error: &str,
    ) -> wasmtime::Result<(Self, Result<(), String>)> {
        let mut state = State::new();
        state.observed.registrations.error = Some(error.to_owned());
        Box::pin(Self::start_reporting(engine, component, state)).await
    }

    /// Instantiates a component on `state` and runs its factory, failing when the factory fails.
    async fn start_on(
        engine: &Engine,
        component: &Component,
        state: State,
    ) -> wasmtime::Result<Self> {
        let (harness, outcome) = Box::pin(Self::start_reporting(engine, component, state)).await?;
        reported(outcome)?;
        Ok(harness)
    }

    /// Instantiates a component on `state` and runs its factory, returning the harness beside
    /// the factory's own outcome so a refused start can still be observed.
    async fn start_reporting(
        engine: &Engine,
        component: &Component,
        state: State,
    ) -> wasmtime::Result<(Self, Result<(), String>)> {
        let origin = Arc::new(OnceLock::new());
        let linker = component_linker(engine)?;
        let mut store = Store::new(engine, state);
        let extension = Extension::instantiate_async(&mut store, component, &linker).await?;
        let exports = extension.maestro_extension_guest().clone();
        let _ = origin.set(exports.clone());
        store.data_mut().schema_origin = Some(origin);
        store.data_mut().guest = Some(exports.clone());
        let started = exports.clone();
        let outcome = store
            .run_concurrent(async |accessor| started.call_start(accessor).await)
            .await??;
        Ok((Self { store, exports }, outcome))
    }

    /// Start another author instance using the same host resources and store.
    pub async fn add_instance(&mut self, path: &Path) -> wasmtime::Result<Guest> {
        let (_, component) = compiled(path)?;
        let origin = Arc::new(OnceLock::new());
        let linker = component_linker(self.store.engine())?;
        let extension = Extension::instantiate_async(&mut self.store, &component, &linker).await?;
        let exports = extension.maestro_extension_guest().clone();
        let _ = origin.set(exports.clone());
        self.store.data_mut().schema_origin = Some(origin);
        self.store.data_mut().guest = Some(exports.clone());
        reported(
            self.store
                .run_concurrent(async |accessor| exports.call_start(accessor).await)
                .await??,
        )?;
        Ok(exports)
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

    /// The wall clock the component's standard library reads, when the test supplied one.
    pub fn clock(&self) -> Option<Clock> {
        self.store.data().clock.clone()
    }

    /// A fresh ordinary context in `cwd`.
    pub fn ordinary(&mut self, cwd: &str) -> wasmtime::Result<Resource<Ordinary>> {
        {
            let model = self.store.data().observed.catalog.model.clone();
            self.store
                .data_mut()
                .push(Ordinary(Session::new(cwd), model))
        }
    }

    /// A fresh progress resource.
    pub fn update(&mut self) -> wasmtime::Result<Resource<Update>> {
        self.store.data_mut().push(Update)
    }

    /// A fresh identity the component never registered. The host keeps it and only lends
    /// borrows of it, so it is not counted as handed to the extension.
    pub fn identity(&mut self, id: u32) -> wasmtime::Result<u32> {
        Ok(self.store.data_mut().table.push(Identity(id, None))?.rep())
    }

    /// A fresh replacement context in `cwd`.
    pub fn replaced(&mut self, cwd: &str) -> wasmtime::Result<Resource<Replaced>> {
        self.store
            .data_mut()
            .push(Replaced(Session::new(cwd), None))
    }

    /// A fresh command context in `cwd`.
    pub fn command_context(&mut self, cwd: &str) -> wasmtime::Result<Resource<Command>> {
        {
            let model = self.store.data().observed.catalog.model.clone();
            self.store
                .data_mut()
                .push(Command(Session::new(cwd), model, None))
        }
    }

    /// A fresh cancellation flag, already cancelled when `aborted`.
    pub fn flag(&mut self, aborted: bool) -> wasmtime::Result<u32> {
        Ok(self
            .store
            .data_mut()
            .push(Flag(std::sync::Arc::new(
                std::sync::atomic::AtomicBool::new(aborted),
            )))?
            .rep())
    }

    /// Cancels a flag the component may have retained.
    pub fn abort(&mut self, flag: u32) -> wasmtime::Result<()> {
        self.store
            .data_mut()
            .table
            .get_mut(&Resource::<Flag>::new_borrow(flag))?
            .0
            .store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    /// Asks the component to release the callback at table key `rep`.
    pub async fn release(&mut self, rep: u32) -> wasmtime::Result<()> {
        let origin = self.store.data().table.get(&borrow(rep))?.1.clone();
        let guest = match origin {
            Some(super::schema_host::Origin::Guest(origin)) => origin
                .get()
                .cloned()
                .ok_or_else(|| wasmtime::format_err!("schema origin is unavailable"))?,
            None => self.exports.clone(),
        };
        let function = guest
            .func_release()
            .func()
            .typed::<(Resource<Identity>,), ()>(&self.store)?;
        self.store
            .run_concurrent(async |accessor| {
                function.call_concurrent(accessor, (borrow(rep),)).await
            })
            .await?
    }

    /// Releases every registration in the order the host received it, reporting each name with
    /// the outcome of its own release.
    pub async fn release_all(&mut self) -> Vec<(String, wasmtime::Result<()>)> {
        let mut released = Vec::new();
        while let Some((name, key)) = self.store.data_mut().observed.next_to_release() {
            let outcome = self.release(key).await;
            released.push((name, outcome));
        }
        released
    }
}

/// Emits `data` on `channel` through the host bus operation, reaching every registered
/// listener of that channel.
pub fn bus_emit(harness: &mut Harness, channel: &str, data: &str) -> Result<(), String> {
    let channel = channel.to_owned();
    let data = data.to_owned();
    super::block_on(harness.store.run_concurrent(async |accessor| {
        // The concurrent accessor projects the state as itself; the import runs as Imports.
        let imports = accessor.with_getter::<Imports>(|state| state);
        host::HostWithStore::event_emit(&imports, channel, data).await
    }))
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())?
}

/// Cancels the retained tool signal after its entered barrier and releases execution.
pub async fn cancel_held(
    accessor: &Accessor<State>,
    held: Option<(oneshot::Receiver<()>, oneshot::Sender<()>)>,
    flag: Option<u32>,
) {
    let Some((entered, open)) = held else { return };
    entered.await.expect("execution entered its wait");
    if let Some(flag) = flag {
        accessor
            .with(|mut a| a.get().abort(flag))
            .expect("retained flag exists");
    }
    open.send(()).expect("execution still pending");
}

impl host::HostWithStore<State> for Imports {
    fn fetch(
        accessor: &Accessor<State, Self>,
        request: host::HttpRequest,
        signal: Option<Resource<Flag>>,
    ) -> impl Future<
        Output = wasmtime::Result<Result<Resource<super::native_http_host::Response>, String>>,
    > {
        std::future::ready(super::native_http_host::fetch(accessor, request, signal))
    }
    fn spawn(
        accessor: &Accessor<State, Self>,
        command: String,
        args: Vec<String>,
        options: host::SpawnOptions,
    ) -> impl Future<Output = wasmtime::Result<Result<Resource<super::native_io_host::Child>, String>>>
    {
        std::future::ready(super::native_io_host::spawn(
            accessor, command, args, options,
        ))
    }

    async fn sleep(
        accessor: &Accessor<State, Self>,
        duration: (u64, u32),
        signal: Option<Resource<Flag>>,
    ) -> wasmtime::Result<Result<(), String>> {
        let aborted = |a: &Accessor<State, Self>| {
            a.with(|mut a| {
                signal
                    .as_ref()
                    .is_some_and(|s| a.get().table.get(s).unwrap().0.load(Ordering::SeqCst))
            })
        };
        if aborted(accessor) {
            return Ok(Err("Aborted".into()));
        }
        let hold = accessor.with(|mut a| {
            a.get()
                .observed
                .log(format!("native-wait {} {}", duration.0, duration.1));
            a.get().observed.take_hold()
        });
        let clock = accessor.with(|mut access| access.get().observed.tick_clock.clone());
        if let Some(clock) = clock {
            clock.wait().await;
        } else {
            pause(hold).await;
        }
        Ok(if aborted(accessor) {
            Err("Aborted".into())
        } else {
            Ok(())
        })
    }

    async fn call(
        accessor: &Accessor<State, Self>,
        operation: String,
        data: String,
        _signal: Option<Resource<Flag>>,
    ) -> wasmtime::Result<Result<String, String>> {
        let data: serde_json::Value = serde_json::from_str(&data)?;
        let started = accessor.with(|mut access| {
            super::action_host::action(&mut access.get().observed, &operation, &data)
        });
        if let Err(error) = started {
            return Ok(Err(error));
        }
        let hold = accessor.with(|mut access| access.get().observed.take_hold());
        pause(hold).await;
        Ok(accessor.with(|mut access| access.get().observed.actions.record()))
    }

    fn register_tool(
        accessor: &Accessor<State, Self>,
        metadata: String,
        prepare: Option<Resource<Identity>>,
        execute: Resource<Identity>,
        schemas: Vec<(String, Resource<Identity>)>,
        render_call: Option<Resource<Identity>>,
        render_result: Option<Resource<Identity>>,
    ) -> impl std::future::Future<Output = wasmtime::Result<Resource<Route>>> {
        std::future::ready(accessor.with(|mut access| {
            let state = access.get();
            let name = super::schema_host::tool_name(&metadata)?;
            state.observed.schemas.retain(|(tool, _, _)| tool != &name);
            for (pointer, handler) in schemas {
                state.table.get_mut(&handler)?.1 = state
                    .schema_origin
                    .clone()
                    .map(super::schema_host::Origin::Guest);
                state
                    .observed
                    .schemas
                    .push((name.clone(), pointer, handler.rep()));
            }
            let rep = |handler: Option<Resource<Identity>>| handler.map(|handler| handler.rep());
            let result = state.observed.register_tool(
                &metadata,
                (rep(prepare), execute.rep()),
                (rep(render_call), rep(render_result)),
            );
            state.routed(result)
        }))
    }

    async fn get_all_tools(
        accessor: &Accessor<State, Self>,
    ) -> wasmtime::Result<Result<host::ToolListing, String>> {
        super::schema_host::get_all_tools(accessor).await
    }

    async fn set_model(
        accessor: &Accessor<State, Self>,
        model: String,
    ) -> wasmtime::Result<Result<bool, String>> {
        accessor.with(|mut access| access.get().observed.log("model selection started"));
        let hold = accessor.with(|mut access| access.get().observed.take_hold());
        pause(hold).await;
        Ok(accessor.with(|mut access| {
            let state = access.get();
            super::action_host::action(
                &mut state.observed,
                "setModel",
                &serde_json::json!([serde_json::from_str::<serde_json::Value>(&model).unwrap()]),
            )?;
            state.observed.actions.boolean("setModel")
        }))
    }

    async fn event_emit(
        accessor: &Accessor<State, Self>,
        channel: String,
        data: String,
    ) -> wasmtime::Result<Result<(), String>> {
        let hold = accessor.with(|mut access| access.get().observed.take_hold());
        pause(hold).await;
        let emitter = accessor
            .with(|mut access| access.get().table.push(super::bus_host::Emitter { key: 0 }))?;
        let result =
            Self::emit_listeners(accessor, Resource::new_borrow(emitter.rep()), channel, data)
                .await;
        accessor.with(|mut access| access.get().table.delete(emitter))?;
        result
    }
}

impl Imports {
    async fn emit_listeners(
        accessor: &Accessor<State, Self>,
        emitter: Resource<super::bus_host::Emitter>,
        channel: String,
        data: String,
    ) -> wasmtime::Result<Result<(), String>> {
        let (guest, handlers) = accessor.with(|mut access| {
            let state = access.get();
            let key = state.table.get(&emitter).expect("borrowed emitter").key;
            if key != 0 {
                state.observed.bus.renderer_emissions.push((
                    key,
                    channel.clone(),
                    serde_json::from_str(&data).expect("encoded payload"),
                ));
            }
            (
                state.guest.clone(),
                state.observed.bus.emit(&channel, &data),
            )
        });
        let handlers = match handlers {
            Ok(handlers) => handlers,
            Err(error) => return Ok(Err(error)),
        };
        let guest = guest.ok_or_else(|| wasmtime::format_err!("no guest recorded"))?;
        for handler in handlers {
            let function = accessor.with(|access| {
                guest.func_invoke_listener().func().typed::<(
                    Resource<Identity>,
                    String,
                    Resource<super::bus_host::Emitter>,
                ), (Result<(), String>,)>(access)
            })?;
            let (result,) = function
                .call_concurrent(
                    accessor,
                    (
                        borrow(handler),
                        data.clone(),
                        Resource::new_borrow(emitter.rep()),
                    ),
                )
                .await?;
            if let Err(error) = result {
                return Ok(Err(error));
            }
        }
        Ok(Ok(()))
    }
}
