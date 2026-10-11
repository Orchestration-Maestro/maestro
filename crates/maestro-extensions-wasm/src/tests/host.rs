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
pub struct Flag(pub std::sync::Arc<std::sync::atomic::AtomicBool>);
/// Progress resource owned by one execution.
pub struct Update;

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

/// Host state shared by every import.
pub struct State {
    /// WASI state the component's standard library needs.
    wasi: WasiCtx,
    /// Host-side resources by handle.
    pub table: ResourceTable,
    /// What the host observed.
    pub observed: Observed,
    /// The guest, once instantiated, so imports can call back into it.
    pub(super) guest: Option<Guest>,
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

    /// Cancels a retained resource by its table key.
    pub fn abort(&mut self, flag: u32) -> wasmtime::Result<()> {
        self.table
            .get_mut(&Resource::<Flag>::new_borrow(flag))?
            .0
            .store(true, std::sync::atomic::Ordering::SeqCst);
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
    ) -> wasmtime::Result<Result<(), String>> {
        let value: serde_json::Value = serde_json::from_str(&partial)?;
        self.observed.updates.push(partial);
        Ok(if value["details"] == "fail update" {
            Err("failed update: Ω".into())
        } else {
            Ok(())
        })
    }
    fn drop(&mut self, this: Resource<Update>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

impl host::HostAbortSignal for State {
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
        ) -> wasmtime::Result<Result<Resource<super::catalog_host::Ui>, String>> {
            let _ = &this;
            if let Err(e) = self.observed.catalog.check() {
                return Ok(Err(e));
            }
            Ok(Ok(self.push(super::catalog_host::Ui)?))
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
        fn abort(&mut self, this: Resource<$kind>) -> wasmtime::Result<Result<(), String>> {
            let _ = &this;
            if let Err(e) = self.observed.catalog.check() {
                return Ok(Err(e));
            }
            self.observed.log("context abort");
            Ok(Ok(()))
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
        fn shutdown(&mut self, this: Resource<$kind>) -> wasmtime::Result<Result<(), String>> {
            let _ = &this;
            if let Err(e) = self.observed.catalog.check() {
                return Ok(Err(e));
            }
            self.observed.log("context shutdown");
            Ok(Ok(()))
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
            instructions: Option<String>,
            complete: Option<Resource<Identity>>,
            error: Option<Resource<Identity>>,
        ) -> wasmtime::Result<Result<(), String>> {
            if let Err(error) = self.observed.actions.check() {
                return Ok(Err(error));
            }
            let cwd = match self.table.get(&this)?.0.cwd() {
                Ok(cwd) => cwd,
                Err(error) => return Ok(Err(error)),
            };
            self.observed.log(format!("compact binding {cwd}"));
            Ok(super::action_host::compact(
                &mut self.observed,
                instructions.as_deref(),
                complete.map(|c| c.rep()),
                error.map(|c| c.rep()),
            ))
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
impl host::HostTheme for State {
    fn drop(&mut self, this: Resource<super::renderer_host::Theme>) -> wasmtime::Result<()> {
        let key = self.table.get(&this)?.0.clone();
        self.observed.log(format!("theme-drop {key}"));
        self.release(this)
    }
}
impl host::HostCallbackEmitter for State {
    fn drop(&mut self, this: Resource<super::bus_host::Emitter>) -> wasmtime::Result<()> {
        self.table.delete(this)?;
        Ok(())
    }
}
impl host::Host for State {
    fn register_message_renderer(
        &mut self,
        custom_type: String,
        handler: Resource<Identity>,
    ) -> wasmtime::Result<Result<(), String>> {
        if let Some(error) = &self.observed.registrations.error {
            return Ok(Err(error.clone()));
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
    ) -> wasmtime::Result<Result<(), String>> {
        Ok(super::action_host::action(
            &mut self.observed,
            "sendMessage",
            &serde_json::json!([
                super::action_host::document(Some(&message)),
                super::action_host::document(options.as_deref())
            ]),
        ))
    }
    fn send_user_message(
        &mut self,
        content: String,
        options: Option<String>,
    ) -> wasmtime::Result<Result<(), String>> {
        Ok(super::action_host::action(
            &mut self.observed,
            "sendUserMessage",
            &serde_json::json!([
                super::action_host::document(Some(&content)),
                super::action_host::document(options.as_deref())
            ]),
        ))
    }
    fn set_session_name(&mut self, name: String) -> wasmtime::Result<Result<(), String>> {
        Ok(super::action_host::action(
            &mut self.observed,
            "setSessionName",
            &serde_json::json!([name]),
        ))
    }
    fn set_label(
        &mut self,
        entry_id: String,
        label: Option<String>,
    ) -> wasmtime::Result<Result<(), String>> {
        Ok(super::action_host::action(
            &mut self.observed,
            "setLabel",
            &serde_json::json!([
                entry_id,
                label.map_or(
                    serde_json::json!({"present":false}),
                    |v| serde_json::json!({"present":true,"value":v})
                )
            ]),
        ))
    }
    fn set_active_tools(
        &mut self,
        tool_names: Vec<String>,
    ) -> wasmtime::Result<Result<(), String>> {
        Ok(super::action_host::action(
            &mut self.observed,
            "setActiveTools",
            &serde_json::json!([tool_names]),
        ))
    }
    fn set_thinking_level(&mut self, level: String) -> wasmtime::Result<Result<(), String>> {
        Ok(super::action_host::action(
            &mut self.observed,
            "setThinkingLevel",
            &serde_json::json!([serde_json::from_str::<serde_json::Value>(&level).unwrap()]),
        ))
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
            serde_json::from_value(self.observed.actions.value.clone()).map_err(|e| e.to_string())
        }))
    }
    fn get_all_tools(&mut self) -> wasmtime::Result<Result<String, String>> {
        Ok(self.observed.actions.record())
    }
    fn get_commands(&mut self) -> wasmtime::Result<Result<String, String>> {
        Ok(self.observed.actions.record())
    }
    fn get_thinking_level(&mut self) -> wasmtime::Result<Result<String, String>> {
        Ok(self.observed.actions.record())
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
        if let Some(error) = &self.observed.registrations.error {
            return Ok(Err(error.clone()));
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
        if let Some(error) = &self.observed.registrations.error {
            return Ok(Err(error.clone()));
        }
        self.observed
            .registrations
            .shortcuts
            .push((shortcut.clone(), description));
        self.observed
            .register(format!("shortcut {shortcut}"), handler.rep());
        Ok(Ok(()))
    }
    fn register_flag(
        &mut self,
        name: String,
        options: host::FlagOptions,
    ) -> wasmtime::Result<Result<(), String>> {
        if let Some(error) = &self.observed.registrations.error {
            return Ok(Err(error.clone()));
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

    fn register_tool(
        &mut self,
        metadata: String,
        prepare: Option<Resource<Identity>>,
        execute: Resource<Identity>,
    ) -> wasmtime::Result<Result<(), String>> {
        Ok(self
            .observed
            .register_tool(&metadata, prepare.map(|p| p.rep()), execute.rep()))
    }

    fn append_entry(
        &mut self,
        custom_type: String,
        data: Option<String>,
    ) -> wasmtime::Result<Result<(), String>> {
        if custom_type == "state Ω"
            && let Err(error) = self.observed.actions.check()
        {
            return Ok(Err(error));
        }
        self.observed.entry(&custom_type, data.as_deref());
        Ok(Ok(()))
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
        callback: Option<Resource<Identity>>,
    ) -> wasmtime::Result<Result<SessionChangeResult, String>> {
        super::replacement_host::binding(accessor, &this)?;
        super::replacement_host::command(accessor, "switchSession", &id, None, callback).await
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
        data: NewSessionCommandData,
        setup: Option<Resource<Identity>>,
        with_session: Option<Resource<Identity>>,
    ) -> wasmtime::Result<Result<SessionChangeResult, String>> {
        let parent = data.parent_session.as_deref().unwrap_or("none");
        let hold = accessor.with(|mut a| {
            super::setup_host::command_start(
                &mut a.get().observed,
                data.parent_session.as_deref(),
                (setup.is_some(), with_session.is_some()),
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
        Ok(self.store.data_mut().table.push(Identity(id))?.rep())
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
        self.exports
            .func_release()
            .call_async(&mut self.store, (borrow(rep),))
            .await
    }
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
            state.observed.actions.value.as_bool().ok_or_else(|| {
                state
                    .observed
                    .actions
                    .value
                    .as_str()
                    .unwrap_or("missing boolean reply")
                    .to_owned()
            })
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
            Self::callback_emit(accessor, Resource::new_borrow(emitter.rep()), channel, data).await;
        accessor.with(|mut access| access.get().table.delete(emitter))?;
        result
    }
    async fn callback_emit(
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
