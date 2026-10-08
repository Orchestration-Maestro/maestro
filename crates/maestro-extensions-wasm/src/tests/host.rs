//! A test-only native host: it instantiates the built author component and implements the
//! imports the component calls.
use std::collections::HashMap;
use std::future::Future;

use tokio::sync::oneshot;
use wasmtime::component::{Accessor, Component, HasData, Linker, Resource, ResourceTable};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder, WasiCtxView, WasiView};

use super::controlled::{Hold, STALE};
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
/// Progress channel of one tool call.
pub struct Progress(String);
/// Emitter a synchronous callback borrows.
pub struct Emitter;
/// Host-owned user interface.
pub struct Ui;
/// Host-owned theme.
pub struct ThemeHandle;
/// Host-owned model catalog.
pub struct Catalog;
/// Host-owned session tree node.
pub struct Node;
/// Host-owned read access to a session.
pub struct Reader;
/// Host-owned write access to a session.
pub struct Writer;
/// Host-owned event bus.
pub struct Bus;
/// Host-owned bus subscription.
pub struct Subscribed;
/// Session state; each context kind wraps it in its own host type.
pub struct Session {
    /// Working directory.
    cwd: String,
    /// Whether a replacement made the session stale.
    stale: bool,
}
/// Host type of the ordinary context resource.
pub struct Ordinary(Session);
/// Host type of the command context resource.
pub struct Command(Session);
/// Host type of the replacement context resource.
pub struct Replaced(Session);

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

/// Host state shared by every import.
pub struct State {
    /// WASI state the standard library needs.
    wasi: WasiCtx,
    /// Resources handed to the component.
    pub table: ResourceTable,
    /// Observations in time order.
    pub transcript: Vec<String>,
    /// Identities the component dropped, in drop order.
    pub dropped: Vec<u32>,
    /// Source of fresh identities.
    next: u32,
    /// Table keys of the registered callbacks by registration key.
    pub registered: HashMap<String, u32>,
    /// Registration keys in registration order.
    pub order: Vec<String>,
    /// Listener tails the guest announced, as listener and tail table keys.
    pub tails: Vec<(u32, u32)>,
    /// Holds the next new session pending, when set.
    hold: Option<Hold>,
    /// The instantiated exports, for calls the host makes while an import is pending.
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
    /// Empty host state.
    fn new() -> Self {
        Self {
            wasi: WasiCtxBuilder::new().build(),
            table: ResourceTable::new(),
            transcript: Vec::new(),
            dropped: Vec::new(),
            next: 0,
            registered: HashMap::new(),
            order: Vec::new(),
            tails: Vec::new(),
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

    /// Records a call the host answers with a fixed value.
    fn record(&mut self, name: &str, arguments: &[String]) {
        self.log(format!("host {name}({})", arguments.join(", ")));
    }

    /// Remembers which table key a registration announced.
    fn register(&mut self, key: String, callback: &Resource<Identity>) {
        self.order.push(key.clone());
        self.registered.insert(key, callback.rep());
    }
}

/// The record-only interfaces need no host behavior.
macro_rules! record_interfaces {
    ($($interface:ident),*) => { $(impl $interface::Host for State {})* };
}

record_interfaces!(events, models, session);

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

impl host::HostToolUpdate for State {
    fn send(
        &mut self,
        this: Resource<Progress>,
        partial: models::AgentToolResult,
    ) -> wasmtime::Result<Result<(), String>> {
        let call = self.table.get(&this)?.0.clone();
        self.log(format!(
            "tool-update {call} {}",
            super::host_family::describe(&partial.content)
        ));
        Ok(Ok(()))
    }

    fn drop(&mut self, this: Resource<Progress>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

impl host::HostCallbackEmitter for State {
    fn emit(
        &mut self,
        _: Resource<Emitter>,
        channel: String,
        data: String,
    ) -> wasmtime::Result<Result<(), String>> {
        self.log(format!("emit {channel} {data}"));
        Ok(Ok(()))
    }

    fn drop(&mut self, this: Resource<Emitter>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

/// Methods every session-bound context resource shares: the working directory answers from
/// the session, resource-returning operations hand out fresh resources and the rest record
/// their call and answer with a fixed value.
macro_rules! context_methods {
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

        fn ui(&mut self, _: Resource<$kind>) -> wasmtime::Result<Result<Resource<Ui>, String>> {
            Ok(Ok(self.push(Ui)?))
        }

        fn session_manager(
            &mut self,
            _: Resource<$kind>,
        ) -> wasmtime::Result<Result<Resource<Reader>, String>> {
            Ok(Ok(self.push(Reader)?))
        }

        fn model_registry(
            &mut self,
            _: Resource<$kind>,
        ) -> wasmtime::Result<Result<Resource<Catalog>, String>> {
            Ok(Ok(self.push(Catalog)?))
        }

        fn compact(
            &mut self,
            _: Resource<$kind>,
            custom_instructions: Option<String>,
            on_complete: Option<Resource<Identity>>,
            on_error: Option<Resource<Identity>>,
        ) -> wasmtime::Result<Result<(), String>> {
            self.record(
                "compact",
                &[
                    format!("{custom_instructions:?}"),
                    format!("complete={}", on_complete.is_some()),
                    format!("error={}", on_error.is_some()),
                ],
            );
            Ok(Ok(()))
        }

        fn drop(&mut self, this: Resource<$kind>) -> wasmtime::Result<()> {
            self.release(this)
        }

        recorded_methods! { $kind;
            has_ui() -> Result<bool, String> = Ok(true);
            model() -> Result<Option<models::Model>, String> = Ok(None);
            is_idle() -> Result<bool, String> = Ok(true);
            abort() -> Result<(), String> = Ok(());
            has_pending_messages() -> Result<bool, String> = Ok(false);
            shutdown() -> Result<(), String> = Ok(());
            get_context_usage() -> Result<Option<session::ContextUsage>, String> = Ok(None);
            get_system_prompt() -> Result<String, String> = Ok(String::from("system prompt"));
        }
    };
}

/// Records a call of the host's and returns the reply the test host fixed for it.
macro_rules! recorded_methods {
    ($kind:ident; $($name:ident($($arg:ident: $ty:ty),*) -> $ret:ty = $reply:expr;)*) => {
        $(
            fn $name(&mut self, _: Resource<$kind>, $($arg: $ty),*) -> wasmtime::Result<$ret> {
                self.record(stringify!($name), &[$(format!("{:?}", &$arg)),*]);
                Ok($reply)
            }
        )*
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

/// Records a free host function and answers with a fixed value.
macro_rules! recorded_free {
    ($($name:ident($($arg:ident: $ty:ty),*) -> $ret:ty = $reply:expr;)*) => {
        $(
            fn $name(&mut self, $($arg: $ty),*) -> wasmtime::Result<$ret> {
                self.record(stringify!($name), &[$(format!("{:?}", &$arg)),*]);
                Ok($reply)
            }
        )*
    };
}

impl host::Host for State {
    recorded_free! {
        register_flag(name: String, options: host::FlagOptions) -> Result<(), String> = Ok(());
        get_flag(name: String) -> Result<Option<host::FlagValue>, String> = Ok(None);
        send_message(message: models::CustomMessageInput, options: Option<host::SendMessageOptions>) -> Result<(), String> = Ok(());
        send_user_message(content: models::UserContent, options: Option<host::SendUserMessageOptions>) -> Result<(), String> = Ok(());
        set_session_name(name: String) -> Result<(), String> = Ok(());
        get_session_name() -> Result<Option<String>, String> = Ok(None);
        set_label(entry_id: String, label: Option<String>) -> Result<(), String> = Ok(());
        get_active_tools() -> Result<Vec<String>, String> = Ok(Vec::new());
        get_all_tools() -> Result<Vec<session::ToolInfo>, String> = Ok(Vec::new());
        set_active_tools(names: Vec<String>) -> Result<(), String> = Ok(());
        get_commands() -> Result<Vec<session::SlashCommandInfo>, String> = Ok(Vec::new());
        get_thinking_level() -> Result<models::ThinkingLevel, String> = Ok(models::ThinkingLevel::Off);
        set_thinking_level(level: models::ThinkingLevel) -> Result<(), String> = Ok(());
    }

    fn register_shortcut(
        &mut self,
        shortcut: String,
        _description: Option<String>,
        handler: Resource<Identity>,
    ) -> wasmtime::Result<Result<(), String>> {
        self.log(format!("register shortcut {shortcut}"));
        self.register(format!("shortcut {shortcut}"), &handler);
        Ok(Ok(()))
    }

    fn register_message_renderer(
        &mut self,
        custom_type: String,
        renderer: Resource<Identity>,
    ) -> wasmtime::Result<Result<(), String>> {
        self.log(format!("register renderer {custom_type}"));
        self.register(format!("renderer {custom_type}"), &renderer);
        Ok(Ok(()))
    }

    fn events(&mut self) -> wasmtime::Result<Resource<Bus>> {
        self.push(Bus)
    }

    fn announce_listener_tail(
        &mut self,
        listener: Resource<Identity>,
        tail: Resource<Identity>,
    ) -> wasmtime::Result<()> {
        self.log("listener-tail announced".to_owned());
        self.tails.push((listener.rep(), tail.rep()));
        Ok(())
    }

    fn register_tool(
        &mut self,
        metadata: host::ToolMetadata,
        prepare: Option<Resource<Identity>>,
        execute: Resource<Identity>,
    ) -> wasmtime::Result<Result<(), String>> {
        self.log(format!(
            "register tool {} prepare={}",
            metadata.name,
            prepare.is_some()
        ));
        if let Some(prepare) = &prepare {
            self.register(format!("tool {} prepare", metadata.name), prepare);
        }
        self.register(format!("tool {} execute", metadata.name), &execute);
        Ok(Ok(()))
    }

    fn on(
        &mut self,
        event: String,
        handler: Resource<Identity>,
    ) -> wasmtime::Result<Result<(), String>> {
        if event == super::controlled::REJECTED {
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
        _completions: Option<Resource<Identity>>,
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

impl host::HostUiContext for State {
    fn drop(&mut self, this: Resource<Ui>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

impl host::HostTheme for State {
    fn drop(&mut self, this: Resource<ThemeHandle>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

impl host::HostSubscription for State {
    recorded_methods! { Subscribed;
        unsubscribe() -> Result<(), String> = Ok(());
    }

    fn drop(&mut self, this: Resource<Subscribed>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

impl host::HostEventBus for State {
    fn on(
        &mut self,
        _: Resource<Bus>,
        channel: String,
        listener: Resource<Identity>,
    ) -> wasmtime::Result<Result<Resource<Subscribed>, String>> {
        self.record("bus.on", &[format!("{channel:?}")]);
        self.register(format!("listener {channel}"), &listener);
        Ok(Ok(self.push(Subscribed)?))
    }

    fn drop(&mut self, this: Resource<Bus>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

impl host::HostModelRegistry for State {
    recorded_methods! { Catalog;
        get_all() -> Result<Vec<models::Model>, String> = Ok(Vec::new());
        get_available() -> Result<Vec<models::Model>, String> = Ok(Vec::new());
        find(provider: String, model_id: String) -> Result<Option<models::Model>, String> = Ok(None);
    }

    fn drop(&mut self, this: Resource<Catalog>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

impl host::HostSessionTreeNode for State {
    fn children(
        &mut self,
        _: Resource<Node>,
    ) -> wasmtime::Result<Result<Vec<Resource<Node>>, String>> {
        self.record("children", &[]);
        Ok(Ok(vec![self.push(Node)?]))
    }

    recorded_methods! { Node;
        entry() -> Result<session::SessionEntry, String> = Ok(super::host_family::info_entry("node"));
        label() -> Result<Option<String>, String> = Ok(None);
        label_timestamp() -> Result<Option<String>, String> = Ok(None);
    }

    fn drop(&mut self, this: Resource<Node>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

/// Methods of the read-only session resource, shared by the setup session.
macro_rules! reader_methods {
    ($kind:ident) => {
        fn get_tree(
            &mut self,
            _: Resource<$kind>,
        ) -> wasmtime::Result<Result<Vec<Resource<Node>>, String>> {
            self.record("get_tree", &[]);
            Ok(Ok(vec![self.push(Node)?]))
        }

        recorded_methods! { $kind;
            get_cwd() -> Result<String, String> = Ok(String::from("/session"));
            get_session_dir() -> Result<String, String> = Ok(String::from("/sessions"));
            get_session_id() -> Result<String, String> = Ok(String::from("session-id"));
            get_session_file() -> Result<Option<String>, String> = Ok(None);
            get_leaf_id() -> Result<Option<String>, String> = Ok(None);
            get_leaf_entry() -> Result<Option<session::SessionEntry>, String> = Ok(None);
            get_entry(id: String) -> Result<Option<session::SessionEntry>, String> = Ok(None);
            get_label(id: String) -> Result<Option<String>, String> = Ok(None);
            get_branch(from_id: Option<String>) -> Result<Vec<session::SessionEntry>, String> = Ok(Vec::new());
            get_header() -> Result<Option<session::SessionHeader>, String> = Ok(None);
            get_entries() -> Result<Vec<session::SessionEntry>, String> = Ok(Vec::new());
            get_session_name() -> Result<Option<String>, String> = Ok(None);
        }
    };
}

impl host::HostReadonlySessionManager for State {
    reader_methods!(Reader);

    fn drop(&mut self, this: Resource<Reader>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

impl host::HostSessionManager for State {
    fn reader(&mut self, _: Resource<Writer>) -> wasmtime::Result<Resource<Reader>> {
        self.push(Reader)
    }

    recorded_methods! { Writer;
        set_session_file(path: String) -> Result<(), String> = Ok(());
        new_session(options: Option<session::NewSessionOptions>) -> Result<Option<String>, String> = Ok(None);
        is_persisted() -> Result<bool, String> = Ok(false);
        get_children(parent_id: String) -> Result<Vec<session::SessionEntry>, String> = Ok(Vec::new());
        build_session_context() -> Result<session::SessionContext, String> = Ok(super::host_family::empty_session_context());
        append_message(message: models::AgentMessage) -> Result<String, String> = Ok(String::from("appended"));
        append_thinking_level_change(level: String) -> Result<String, String> = Ok(String::from("appended"));
        append_model_change(provider: String, model_id: String) -> Result<String, String> = Ok(String::from("appended"));
        append_compaction(compaction: session::CompactionResult, from_hook: Option<bool>) -> Result<String, String> = Ok(String::from("appended"));
        append_custom_entry(custom_type: String, data: Option<String>) -> Result<String, String> = Ok(String::from("appended"));
        append_session_info(name: String) -> Result<String, String> = Ok(String::from("appended"));
        append_custom_message_entry(message: models::CustomMessageInput) -> Result<String, String> = Ok(String::from("appended"));
        append_label_change(target_id: String, label: Option<String>) -> Result<String, String> = Ok(String::from("appended"));
        branch(from_id: String) -> Result<(), String> = Ok(());
        reset_leaf() -> Result<(), String> = Ok(());
        branch_with_summary(from_id: Option<String>, summary: String, details: Option<String>, from_hook: Option<bool>) -> Result<String, String> = Ok(String::from("appended"));
        create_branched_session(leaf_id: String) -> Result<Option<String>, String> = Ok(None);
    }

    fn drop(&mut self, this: Resource<Writer>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

/// Selects the host state as the data every import runs against.
struct Imports;

impl HasData for Imports {
    type Data<'a> = &'a mut State;
}

impl Imports {
    /// Waits while the driver holds the next session operation pending.
    async fn pending(accessor: &Accessor<State, Self>) -> wasmtime::Result<()> {
        let hold = accessor.with(|mut access| access.get().hold.take());
        if let Some(Hold { started, open }) = hold {
            let _ = started.send(());
            let _ = open.await;
        }
        Ok(())
    }

    /// Runs the guest's one-shot continuation announced for an operation, when there is one.
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

    /// Marks a command context stale and records that the operation finished.
    fn finish(
        accessor: &Accessor<State, Self>,
        this: &Resource<Command>,
        operation: &str,
    ) -> wasmtime::Result<Result<SessionChangeResult, String>> {
        accessor.with(|mut access| -> wasmtime::Result<_> {
            let state = access.get();
            state.table.get_mut(this)?.0.stale = true;
            state.log(format!("{operation} done cancelled=false"));
            Ok(Ok(SessionChangeResult { cancelled: false }))
        })
    }
}

impl host::HostCommandContextWithStore<State> for Imports {
    fn wait_for_idle(
        accessor: &Accessor<State, Self>,
        _: Resource<Command>,
    ) -> impl Future<Output = wasmtime::Result<Result<(), String>>> + Send {
        accessor.with(|mut access| access.get().record("wait_for_idle", &[]));
        std::future::ready(Ok(Ok(())))
    }

    async fn new_session(
        accessor: &Accessor<State, Self>,
        this: Resource<Command>,
        data: NewSessionCommandData,
        setup: Option<Resource<Identity>>,
        with_session: Option<Resource<Identity>>,
    ) -> wasmtime::Result<Result<SessionChangeResult, String>> {
        let parent = data.parent_session.as_deref().unwrap_or("none");
        let line = format!("new-session start parent={parent}");
        accessor.with(|mut access| access.get().log(line));
        Self::pending(accessor).await?;
        if let Some(setup) = setup {
            let (guest, writer) = accessor.with(|mut access| -> wasmtime::Result<_> {
                let state = access.get();
                let guest = state
                    .guest
                    .clone()
                    .ok_or_else(|| wasmtime::format_err!("no guest recorded"))?;
                Ok((guest, state.push(Writer)?))
            })?;
            let borrowed = Resource::new_borrow(setup.rep());
            if let Err(message) = guest.call_invoke_setup(accessor, borrowed, writer).await? {
                return Ok(Err(message));
            }
        }
        if let Err(message) = Self::continue_with(accessor, with_session, "/replacement").await? {
            return Ok(Err(message));
        }
        Self::finish(accessor, &this, "new-session")
    }

    async fn fork(
        accessor: &Accessor<State, Self>,
        this: Resource<Command>,
        entry_id: String,
        data: session::ForkData,
        with_session: Option<Resource<Identity>>,
    ) -> wasmtime::Result<Result<SessionChangeResult, String>> {
        let position = format!("{:?}", data.position);
        let line = format!("fork start entry={entry_id} position={position}");
        accessor.with(|mut access| access.get().log(line));
        Self::pending(accessor).await?;
        if let Err(message) = Self::continue_with(accessor, with_session, "/fork").await? {
            return Ok(Err(message));
        }
        Self::finish(accessor, &this, "fork")
    }

    fn navigate_tree(
        accessor: &Accessor<State, Self>,
        _: Resource<Command>,
        target_id: String,
        options: session::NavigateTreeOptions,
    ) -> impl Future<Output = wasmtime::Result<Result<SessionChangeResult, String>>> + Send {
        let arguments = [format!("{target_id:?}"), format!("{options:?}")];
        accessor.with(|mut access| access.get().record("navigate_tree", &arguments));
        std::future::ready(Ok(Ok(SessionChangeResult { cancelled: false })))
    }

    async fn switch_session(
        accessor: &Accessor<State, Self>,
        this: Resource<Command>,
        path: String,
        with_session: Option<Resource<Identity>>,
    ) -> wasmtime::Result<Result<SessionChangeResult, String>> {
        accessor.with(|mut access| access.get().log(format!("switch start path={path}")));
        Self::pending(accessor).await?;
        if let Err(message) = Self::continue_with(accessor, with_session, &path).await? {
            return Ok(Err(message));
        }
        Self::finish(accessor, &this, "switch-session")
    }

    fn reload(
        accessor: &Accessor<State, Self>,
        _: Resource<Command>,
    ) -> impl Future<Output = wasmtime::Result<Result<(), String>>> + Send {
        accessor.with(|mut access| access.get().record("reload", &[]));
        std::future::ready(Ok(Ok(())))
    }
}

impl host::HostReplacedSessionContextWithStore<State> for Imports {
    fn send_message(
        accessor: &Accessor<State, Self>,
        this: Resource<Replaced>,
        message: models::CustomMessageInput,
        options: Option<host::SendMessageOptions>,
    ) -> impl Future<Output = wasmtime::Result<Result<(), String>>> + Send {
        std::future::ready(accessor.with(|mut access| -> wasmtime::Result<_> {
            let state = access.get();
            let cwd = state.table.get(&this)?.0.cwd.clone();
            state.log(format!(
                "replacement-message {cwd} {:?} {options:?}",
                message.custom_type
            ));
            Ok(Ok(()))
        }))
    }

    fn send_user_message(
        accessor: &Accessor<State, Self>,
        this: Resource<Replaced>,
        content: models::UserContent,
        _options: Option<host::SendUserMessageOptions>,
    ) -> impl Future<Output = wasmtime::Result<Result<(), String>>> + Send {
        let text = match content {
            models::UserContent::Text(text) => text,
            models::UserContent::Blocks(blocks) => format!("{} blocks", blocks.len()),
        };
        std::future::ready(accessor.with(|mut access| -> wasmtime::Result<_> {
            let state = access.get();
            let cwd = state.table.get(&this)?.0.cwd.clone();
            state.log(format!("user-message {cwd} {text}"));
            Ok(Ok(()))
        }))
    }
}

impl host::HostEventBusWithStore<State> for Imports {
    fn emit(
        accessor: &Accessor<State, Self>,
        _: Resource<Bus>,
        channel: String,
        data: String,
    ) -> impl Future<Output = wasmtime::Result<Result<(), String>>> + Send {
        accessor.with(|mut access| access.get().log(format!("bus-emit {channel} {data}")));
        std::future::ready(Ok(Ok(())))
    }
}

impl host::HostModelRegistryWithStore<State> for Imports {
    fn get_api_key_and_headers(
        accessor: &Accessor<State, Self>,
        _: Resource<Catalog>,
        model: models::Model,
    ) -> impl Future<Output = wasmtime::Result<Result<session::ResolvedRequestAuth, String>>> + Send
    {
        let arguments = [format!("{:?}", model.id)];
        accessor.with(|mut access| access.get().record("get_api_key_and_headers", &arguments));
        std::future::ready(Ok(Ok(super::host_family::empty_credentials())))
    }
}

impl host::HostWithStore<State> for Imports {
    fn set_model(
        accessor: &Accessor<State, Self>,
        model: models::Model,
    ) -> impl Future<Output = wasmtime::Result<Result<bool, String>>> + Send {
        let arguments = [format!("{:?}", model.id)];
        accessor.with(|mut access| access.get().record("set_model", &arguments));
        std::future::ready(Ok(Ok(true)))
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
    /// The store holding the host state.
    pub store: Store<State>,
    /// The component's exports.
    pub exports: Guest,
}

impl Harness {
    /// Instantiates the component at `path` and runs its factory.
    ///
    /// # Errors
    /// Returns the engine's or the factory's error.
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

    /// The table key of the callback registered under `key`.
    ///
    /// # Errors
    /// Returns an error naming the key when nothing registered under it.
    pub fn callback(&self, key: &str) -> wasmtime::Result<u32> {
        self.store
            .data()
            .registered
            .get(key)
            .copied()
            .ok_or_else(|| wasmtime::format_err!("nothing registered as {key}"))
    }

    /// Records a line the driver observed.
    pub fn note(&mut self, line: String) {
        self.store.data_mut().transcript.push(line);
    }

    /// Makes the next new session wait for the driver.
    pub fn hold_next_session(&mut self, hold: Hold) {
        self.store.data_mut().hold = Some(hold);
    }

    /// Adds a fresh ordinary context to the table.
    ///
    /// # Errors
    /// Returns the table's error.
    pub fn ordinary(&mut self, cwd: &str) -> wasmtime::Result<Resource<Ordinary>> {
        self.store.data_mut().push(Ordinary(Session::new(cwd)))
    }

    /// Adds an identity the guest never announced and returns its table key.
    ///
    /// # Errors
    /// Returns the table's error.
    pub fn identity(&mut self, id: u32) -> wasmtime::Result<u32> {
        Ok(self.store.data_mut().push(Identity(id))?.rep())
    }

    /// Adds a fresh replacement context to the table.
    ///
    /// # Errors
    /// Returns the table's error.
    pub fn replaced(&mut self, cwd: &str) -> wasmtime::Result<Resource<Replaced>> {
        self.store.data_mut().push(Replaced(Session::new(cwd)))
    }

    /// Adds a fresh command context to the table.
    ///
    /// # Errors
    /// Returns the table's error.
    pub fn command_context(&mut self, cwd: &str) -> wasmtime::Result<Resource<Command>> {
        self.store.data_mut().push(Command(Session::new(cwd)))
    }

    /// Adds the emitter a synchronous callback borrows and returns its key.
    ///
    /// # Errors
    /// Returns the table's error.
    pub fn emitter(&mut self) -> wasmtime::Result<u32> {
        Ok(self.store.data_mut().push(Emitter)?.rep())
    }

    /// Adds the progress channel of a tool call to the table.
    ///
    /// # Errors
    /// Returns the table's error.
    pub fn progress(&mut self, call_id: &str) -> wasmtime::Result<Resource<Progress>> {
        self.store.data_mut().push(Progress(call_id.to_owned()))
    }

    /// Adds a cancellation flag to the table and returns its key.
    ///
    /// # Errors
    /// Returns the table's error.
    pub fn flag(&mut self, aborted: bool) -> wasmtime::Result<u32> {
        Ok(self.store.data_mut().push(Flag(aborted))?.rep())
    }

    /// Flips a flag after the callback that received it returned.
    ///
    /// # Errors
    /// Returns the table's error.
    pub fn abort(&mut self, flag: u32) -> wasmtime::Result<()> {
        self.store
            .data_mut()
            .table
            .get_mut(&Resource::<Flag>::new_borrow(flag))?
            .0 = true;
        Ok(())
    }

    /// Releases the callback registered under `key`.
    ///
    /// # Errors
    /// Returns the engine's error.
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
