//! A minimal in-memory adapter that implements the author ports without a component.
//!
//! It is test support: it records what the author facade forwards, answers port calls with
//! the replies a test scripted, hands out the contexts a host would, and holds an operation
//! pending until the driver has observed it.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use maestro_extensions_wasm::{
    AbortSignal, CallbackEmitterPort, CommandContextPort, CommandHandler, CommandOptions,
    CompactOptions, ContextPort, EventBus, EventBusPort, EventListener, ExtensionAPI,
    ExtensionCommandContext, ExtensionContext, ExtensionEvent, ExtensionEventResult,
    ExtensionFuture, ExtensionHandler, ExtensionHost, ExtensionResult, ExtensionUIContext,
    FlagOptions, FlagValue, ForkOptions, MessageRenderer, Model, ModelRegistry,
    NewSessionCommandOptions, PrepareArguments, ReadonlySessionManager, ReplacedSessionContext,
    ReplacedSessionContextPort, SessionChangeResult, SessionManager, ShortcutOptions, SignalPort,
    Subscription, SubscriptionPort, SwitchSessionOptions, ToolDefinition, ToolExecute,
};
use serde_json::Value;
use tokio::sync::oneshot;

/// The event name the hosts of these tests refuse to register handlers for.
pub const REJECTED: &str = "rejected";

/// Message a context reports after its session was replaced.
pub const STALE: &str = "This context is stale after session replacement.";

/// The observations of one adapter.
#[derive(Clone, Default)]
pub struct Log(Rc<RefCell<Vec<String>>>);

impl Log {
    /// Records one line.
    pub fn push(&self, line: impl Into<String>) {
        self.0.borrow_mut().push(line.into());
    }

    /// The lines recorded so far.
    pub fn lines(&self) -> Vec<String> {
        self.0.borrow().clone()
    }
}

/// A cancellation flag the driver can flip.
#[derive(Clone, Default)]
pub struct Flag(Rc<Cell<bool>>);

impl Flag {
    /// A signal facade over this flag.
    pub fn signal(&self) -> AbortSignal {
        AbortSignal::new(Rc::new(self.clone()))
    }

    /// Cancels the operation.
    pub fn abort(&self) {
        self.0.set(true);
    }
}

impl SignalPort for Flag {
    fn aborted(&self) -> bool {
        self.0.get()
    }
}

/// Answers port calls with scripted replies and records every call.
#[derive(Clone, Default)]
pub struct Script {
    /// The adapter's observations.
    log: Log,
    /// Scripted replies by method name.
    replies: Rc<RefCell<HashMap<&'static str, Rc<dyn Any>>>>,
}

impl Script {
    /// A script that writes its calls to `log`.
    pub fn new(log: Log) -> Self {
        Self {
            log,
            replies: Rc::default(),
        }
    }

    /// Scripts the reply of a method: a value or the host's error message.
    pub fn reply<T: 'static>(&self, method: &'static str, reply: ExtensionResult<T>) {
        self.replies.borrow_mut().insert(method, Rc::new(reply));
    }

    /// The calls recorded so far, in call order.
    pub fn calls(&self) -> Vec<String> {
        self.log.lines()
    }

    /// The scripted reply of a method, without recording a call.
    pub fn peek<T: Clone + 'static>(&self, method: &str) -> Option<ExtensionResult<T>> {
        let reply = self.replies.borrow().get(method).cloned()?;
        reply.downcast_ref::<ExtensionResult<T>>().cloned()
    }

    /// Records a call and answers it from the script.
    pub fn answer<T: Clone + 'static>(
        &self,
        method: &str,
        arguments: &[String],
    ) -> ExtensionResult<T> {
        self.log.push(format!("{method}({})", arguments.join(", ")));
        let reply = self.replies.borrow().get(method).cloned();
        match reply {
            Some(reply) => reply
                .downcast_ref::<ExtensionResult<T>>()
                .cloned()
                .unwrap_or_else(|| Err(format!("the scripted reply of {method} has another type"))),
            None => Err(format!("no scripted reply for {method}")),
        }
    }
}

/// Implements port methods by recording the call and answering from `$script`.
macro_rules! scripted {
    ($script:tt; $(fn $method:ident($($arg:ident: $ty:ty),*) -> $ret:ty;)*) => {
        $(
            fn $method(&self $(, $arg: $ty)*) -> ExtensionResult<$ret> {
                let arguments = [$(format!("{:?}", &$arg)),*];
                self.$script.answer(stringify!($method), &arguments)
            }
        )*
    };
}

/// The state of one session.
struct Session {
    /// Working directory.
    cwd: String,
    /// Whether a replacement made the session stale.
    stale: Cell<bool>,
}

impl Session {
    /// A live session working in `cwd`.
    fn new(cwd: &str) -> Rc<Self> {
        Rc::new(Self {
            cwd: cwd.to_owned(),
            stale: Cell::new(false),
        })
    }

    /// The working directory, or the stale message.
    fn cwd(&self) -> ExtensionResult<String> {
        if self.stale.get() {
            Err(STALE.to_owned())
        } else {
            Ok(self.cwd.clone())
        }
    }
}

/// Lets the driver observe a pending operation and release it.
pub struct Hold {
    /// Fires once the operation is pending.
    pub started: oneshot::Sender<()>,
    /// Resolves when the driver let the operation continue.
    pub open: oneshot::Receiver<()>,
}

/// Ordinary context of a session.
pub struct Ordinary {
    /// The session the context is bound to.
    session: Rc<Session>,
    /// Scripted answers of the operations beyond the working directory.
    script: Script,
}

impl ContextPort for Ordinary {
    fn cwd(&self) -> ExtensionResult<String> {
        self.session.cwd()
    }

    fn signal(&self) -> ExtensionResult<Option<AbortSignal>> {
        self.script.answer("signal", &[])
    }

    scripted! { script;
        fn ui() -> ExtensionUIContext;
        fn has_ui() -> bool;
        fn session_manager() -> ReadonlySessionManager;
        fn model_registry() -> ModelRegistry;
        fn model() -> Option<Model>;
        fn is_idle() -> bool;
        fn abort() -> ();
        fn has_pending_messages() -> bool;
        fn shutdown() -> ();
        fn get_context_usage() -> Option<maestro_extensions_wasm::ContextUsage>;
        fn get_system_prompt() -> String;
    }

    fn compact(&self, options: Option<CompactOptions>) -> ExtensionResult<()> {
        let described = options.map(|options| {
            format!(
                "instructions={:?} complete={} error={}",
                options.custom_instructions,
                options.on_complete.is_some(),
                options.on_error.is_some()
            )
        });
        self.script.answer("compact", &[format!("{described:?}")])
    }
}

/// Command context of a session.
pub struct Command {
    /// The session the context is bound to.
    session: Rc<Session>,
    /// The adapter's observations.
    log: Log,
    /// Scripted answers of the operations beyond the working directory.
    script: Script,
    /// Holds the next session operation pending, when set.
    hold: Rc<RefCell<Option<Hold>>>,
}

impl ContextPort for Command {
    fn cwd(&self) -> ExtensionResult<String> {
        self.session.cwd()
    }

    fn signal(&self) -> ExtensionResult<Option<AbortSignal>> {
        self.script.answer("signal", &[])
    }

    scripted! { script;
        fn ui() -> ExtensionUIContext;
        fn has_ui() -> bool;
        fn session_manager() -> ReadonlySessionManager;
        fn model_registry() -> ModelRegistry;
        fn model() -> Option<Model>;
        fn is_idle() -> bool;
        fn abort() -> ();
        fn has_pending_messages() -> bool;
        fn shutdown() -> ();
        fn get_context_usage() -> Option<maestro_extensions_wasm::ContextUsage>;
        fn get_system_prompt() -> String;
    }

    fn compact(&self, _options: Option<CompactOptions>) -> ExtensionResult<()> {
        self.script.answer("compact", &[])
    }
}

impl Command {
    /// Holds the operation pending when the driver asked for it.
    async fn pending(&self) {
        let hold = self.hold.borrow_mut().take();
        if let Some(Hold { started, open }) = hold {
            let _ = started.send(());
            let _ = open.await;
        }
    }

    /// Finishes an operation: runs the setup, switches to the replacement session named by
    /// the operation's scripted reply `target`, runs the continuation and marks this
    /// session stale.
    async fn replace(
        &self,
        operation: &str,
        setup: Option<maestro_extensions_wasm::SetupSession>,
        with_session: Option<maestro_extensions_wasm::WithSession>,
    ) -> ExtensionResult<SessionChangeResult> {
        self.pending().await;
        let outcome = self.script.peek::<SessionChangeResult>(operation);
        if let Some(Ok(SessionChangeResult { cancelled: true })) = outcome {
            self.log.push(format!(
                "{} done cancelled=true",
                operation.replace('_', "-")
            ));
            return Ok(SessionChangeResult { cancelled: true });
        }
        let target = self
            .script
            .peek::<String>(&format!("{operation}_target"))
            .and_then(Result::ok);
        let target = target.unwrap_or_else(|| "/replacement".to_owned());
        if let Some(setup) = setup {
            setup(SessionManager::new(Rc::new(Writer(self.script.clone())))).await?;
        }
        let replacement = Replaced(Session::new(&target), self.log.clone(), self.script.clone());
        if let Some(with_session) = with_session {
            with_session(ReplacedSessionContext::new(Rc::new(replacement))).await?;
        }
        self.session.stale.set(true);
        self.log.push(format!(
            "{} done cancelled=false",
            operation.replace('_', "-")
        ));
        Ok(SessionChangeResult { cancelled: false })
    }
}

impl CommandContextPort for Command {
    fn wait_for_idle(&self) -> ExtensionFuture<'_, ()> {
        Box::pin(async move { self.script.answer("wait_for_idle", &[]) })
    }

    fn new_session(
        &self,
        options: Option<NewSessionCommandOptions>,
    ) -> ExtensionFuture<'_, SessionChangeResult> {
        Box::pin(async move {
            let options = options.ok_or("the scenario always supplies options")?;
            let parent = options.data.parent_session.as_deref().unwrap_or("none");
            self.log.push(format!("new-session start parent={parent}"));
            self.replace("new_session", options.setup, options.with_session)
                .await
        })
    }

    fn fork<'a>(
        &'a self,
        entry_id: &'a str,
        options: Option<ForkOptions>,
    ) -> ExtensionFuture<'a, SessionChangeResult> {
        Box::pin(async move {
            self.log.push(format!("fork start entry={entry_id}"));
            self.replace(
                "fork",
                None,
                options.and_then(|options| options.with_session),
            )
            .await
        })
    }

    fn navigate_tree<'a>(
        &'a self,
        target_id: &'a str,
        options: Option<maestro_extensions_wasm::NavigateTreeOptions>,
    ) -> ExtensionFuture<'a, SessionChangeResult> {
        Box::pin(async move {
            self.script.answer(
                "navigate_tree",
                &[format!("{target_id:?}"), format!("{options:?}")],
            )
        })
    }

    fn switch_session<'a>(
        &'a self,
        path: &'a str,
        options: Option<SwitchSessionOptions>,
    ) -> ExtensionFuture<'a, SessionChangeResult> {
        Box::pin(async move {
            self.log.push(format!("switch start path={path}"));
            self.replace(
                "switch_session",
                None,
                options.and_then(|options| options.with_session),
            )
            .await
        })
    }

    fn reload(&self) -> ExtensionFuture<'_, ()> {
        Box::pin(async move { self.script.answer("reload", &[]) })
    }
}

/// Context bound to the session an operation created.
pub struct Replaced(Rc<Session>, Log, Script);

impl ReplacedSessionContextPort for Replaced {
    fn command(&self) -> Rc<dyn CommandContextPort> {
        Rc::new(Command {
            session: Rc::clone(&self.0),
            log: self.1.clone(),
            script: self.2.clone(),
            hold: Rc::default(),
        })
    }

    fn send_message(
        &self,
        message: maestro_extensions_wasm::CustomMessageInput,
        options: Option<maestro_extensions_wasm::SendMessageOptions>,
    ) -> ExtensionFuture<'_, ()> {
        let line = format!(
            "replacement-message {} {:?} {options:?}",
            self.0.cwd, message.custom_type
        );
        Box::pin(async move {
            self.1.push(line);
            Ok(())
        })
    }

    fn send_user_message(
        &self,
        content: maestro_extensions_wasm::UserContent,
        _options: Option<maestro_extensions_wasm::SendUserMessageOptions>,
    ) -> ExtensionFuture<'_, ()> {
        let text = match content {
            maestro_extensions_wasm::UserContent::Text(text) => text,
            maestro_extensions_wasm::UserContent::Blocks(blocks) => {
                format!("{} blocks", blocks.len())
            }
        };
        let line = format!("user-message {} {text}", self.0.cwd);
        Box::pin(async move {
            self.1.push(line);
            Ok(())
        })
    }
}

/// A session tree node answering from the script under its own key prefix.
pub struct Node(pub Script, pub &'static str);

impl maestro_extensions_wasm::SessionTreeNodePort for Node {
    fn entry(&self) -> ExtensionResult<maestro_extensions_wasm::SessionEntry> {
        self.0.answer(&format!("{}.entry", self.1), &[])
    }

    fn children(&self) -> ExtensionResult<Vec<maestro_extensions_wasm::SessionTreeNode>> {
        self.0.answer(&format!("{}.children", self.1), &[])
    }

    fn label(&self) -> ExtensionResult<Option<String>> {
        self.0.answer(&format!("{}.label", self.1), &[])
    }

    fn label_timestamp(&self) -> ExtensionResult<Option<String>> {
        self.0.answer(&format!("{}.label_timestamp", self.1), &[])
    }
}

/// Setup writer of a replacement session, answering from the script.
pub struct Writer(pub Script);

impl maestro_extensions_wasm::SessionReaderPort for Writer {
    scripted! { 0;
        fn get_cwd() -> String;
        fn get_session_dir() -> String;
        fn get_session_id() -> String;
        fn get_session_file() -> Option<String>;
        fn get_leaf_id() -> Option<String>;
        fn get_leaf_entry() -> Option<maestro_extensions_wasm::SessionEntry>;
        fn get_entry(id: &str) -> Option<maestro_extensions_wasm::SessionEntry>;
        fn get_label(id: &str) -> Option<String>;
        fn get_branch(from_id: Option<&str>) -> Vec<maestro_extensions_wasm::SessionEntry>;
        fn get_header() -> Option<maestro_extensions_wasm::SessionHeader>;
        fn get_entries() -> Vec<maestro_extensions_wasm::SessionEntry>;
        fn get_tree() -> Vec<maestro_extensions_wasm::SessionTreeNode>;
        fn get_session_name() -> Option<String>;
    }
}

impl maestro_extensions_wasm::SessionWriterPort for Writer {
    scripted! { 0;
        fn set_session_file(path: &str) -> ();
        fn new_session(options: Option<maestro_extensions_wasm::NewSessionOptions>) -> Option<String>;
        fn is_persisted() -> bool;
        fn get_children(parent_id: &str) -> Vec<maestro_extensions_wasm::SessionEntry>;
        fn build_session_context() -> maestro_extensions_wasm::SessionContext;
        fn append_message(message: maestro_extensions_wasm::AgentMessage) -> String;
        fn append_thinking_level_change(level: &str) -> String;
        fn append_model_change(provider: &str, model_id: &str) -> String;
        fn append_compaction(compaction: maestro_extensions_wasm::CompactionResult, from_hook: Option<bool>) -> String;
        fn append_custom_entry(custom_type: &str, data: Option<Value>) -> String;
        fn append_session_info(name: &str) -> String;
        fn append_custom_message_entry(message: maestro_extensions_wasm::CustomMessageInput) -> String;
        fn append_label_change(target_id: &str, label: Option<&str>) -> String;
        fn branch(from_id: &str) -> ();
        fn reset_leaf() -> ();
        fn branch_with_summary(from_id: Option<&str>, summary: &str, details: Option<Value>, from_hook: Option<bool>) -> String;
        fn create_branched_session(leaf_id: &str) -> Option<String>;
    }
}

/// Catalog capability answering from the script.
pub struct Catalog(pub Script);

impl maestro_extensions_wasm::ModelRegistryPort for Catalog {
    scripted! { 0;
        fn get_all() -> Vec<Model>;
        fn get_available() -> Vec<Model>;
        fn find(provider: &str, model_id: &str) -> Option<Model>;
    }

    fn get_api_key_and_headers(
        &self,
        model: Model,
    ) -> ExtensionFuture<'_, maestro_extensions_wasm::ResolvedRequestAuth> {
        Box::pin(async move {
            self.0
                .answer("get_api_key_and_headers", &[format!("{:?}", model.id)])
        })
    }
}

/// Logs what a synchronous callback emits on the bus.
pub struct Emitter(Log);

impl CallbackEmitterPort for Emitter {
    fn emit(&self, channel: &str, data: Value) -> ExtensionResult<()> {
        self.0.push(format!("emit {channel} {data}"));
        Ok(())
    }
}

/// What the controlled host registered.
enum Registration {
    /// An event handler under its event name.
    Event(String, ExtensionHandler),
    /// A command handler under its command name.
    Command(String, CommandOptions),
    /// A tool under its name.
    Tool(String, ToolDefinition),
    /// A keyboard shortcut.
    Shortcut(String, ShortcutOptions),
    /// A custom message renderer under its type.
    Renderer(String, MessageRenderer),
}

/// A registered flag: its name, registration data and current value.
type RegisteredFlag = (String, FlagOptions, Option<FlagValue>);

/// One subscription of the controlled bus.
struct Listener {
    /// The channel it listens on.
    channel: String,
    /// The listener closure.
    listener: EventListener,
}

/// The controlled event bus: it delivers in subscription order and runs the tails after the
/// synchronous prefixes.
pub struct Bus {
    /// The adapter's observations.
    log: Log,
    /// Live subscriptions by identifier.
    listeners: RefCell<Vec<(u32, Listener)>>,
    /// Source of subscription identifiers.
    next: Cell<u32>,
}

impl Bus {
    /// Delivers `data` to the listeners of `channel` and returns their tails.
    fn deliver(&self, channel: &str, data: &Value) -> Vec<ExtensionFuture<'static, ()>> {
        let listeners: Vec<EventListener> = self
            .listeners
            .borrow()
            .iter()
            .filter(|(_, subscribed)| subscribed.channel == channel)
            .map(|(_, subscribed)| Rc::clone(&subscribed.listener))
            .collect();
        let emitter = Emitter(self.log.clone());
        let mut tails = Vec::new();
        for listener in listeners {
            match listener(
                data.clone(),
                maestro_extensions_wasm::CallbackEmitter::new(&emitter),
            ) {
                Ok(Some(tail)) => tails.push(tail),
                Ok(None) => {}
                Err(message) => self.log.push(format!("listener failed: {message}")),
            }
        }
        tails
    }
}

/// Unsubscribes one listener of the controlled bus.
struct Unsubscribe {
    /// The bus.
    bus: Rc<Bus>,
    /// Identifier of the subscription.
    id: u32,
}

impl SubscriptionPort for Unsubscribe {
    fn unsubscribe(&self) -> ExtensionResult<()> {
        self.bus
            .listeners
            .borrow_mut()
            .retain(|(id, _)| *id != self.id);
        self.bus.log.push(format!("unsubscribe {}", self.id));
        Ok(())
    }
}

/// The bus facade port: the bus itself shared with the host.
struct SharedBus(Rc<Bus>);

impl EventBusPort for SharedBus {
    fn emit<'a>(&'a self, channel: &'a str, data: Value) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            self.0.log.push(format!("bus-emit {channel} {data}"));
            for tail in self.0.deliver(channel, &data) {
                tail.await?;
            }
            Ok(())
        })
    }

    fn on(&self, channel: &str, listener: EventListener) -> ExtensionResult<Subscription> {
        let id = self.0.next.get();
        self.0.next.set(id + 1);
        self.0.log.push(format!("subscribe {id} {channel}"));
        let subscribed = Listener {
            channel: channel.to_owned(),
            listener,
        };
        self.0.listeners.borrow_mut().push((id, subscribed));
        Ok(Subscription::new(Rc::new(Unsubscribe {
            bus: Rc::clone(&self.0),
            id,
        })))
    }
}

/// The controlled host adapter.
#[derive(Clone)]
pub struct ControlledHost {
    /// The adapter's observations.
    log: Log,
    /// Scripted answers of the ordinary actions and contexts.
    pub script: Script,
    /// Registrations in registration order.
    registrations: Rc<RefCell<Vec<Registration>>>,
    /// Holds the next session operation pending, when set.
    hold: Rc<RefCell<Option<Hold>>>,
    /// Flags by name with their current value.
    flags: Rc<RefCell<Vec<RegisteredFlag>>>,
    /// The event bus.
    bus: Rc<Bus>,
}

impl ControlledHost {
    /// A host with no registrations, writing to `log`.
    pub fn new(log: Log) -> Self {
        let bus = Rc::new(Bus {
            log: log.clone(),
            listeners: RefCell::default(),
            next: Cell::new(1),
        });
        Self {
            script: Script::new(log.clone()),
            log,
            registrations: Rc::default(),
            hold: Rc::default(),
            flags: Rc::default(),
            bus,
        }
    }

    /// The lines the adapter recorded so far.
    pub fn log_lines(&self) -> Vec<String> {
        self.log.lines()
    }

    /// The calls the scripted ports answered, as recorded lines.
    pub fn script_calls(&self) -> Vec<String> {
        self.log.lines()
    }

    /// The facade an extension factory registers through.
    pub fn api(&self) -> ExtensionAPI {
        ExtensionAPI::new(Rc::new(self.clone()))
    }

    /// Holds the next session operation pending.
    pub fn hold_next_session(&self, hold: Hold) {
        *self.hold.borrow_mut() = Some(hold);
    }

    /// The handler registered for an event name.
    pub fn handler(&self, event: &str) -> Option<ExtensionHandler> {
        self.registrations
            .borrow()
            .iter()
            .find_map(|registration| match registration {
                Registration::Event(name, handler) if name == event => Some(Rc::clone(handler)),
                _ => None,
            })
    }

    /// Every handler registered for an event name, in registration order.
    pub fn handlers(&self, event: &str) -> Vec<ExtensionHandler> {
        self.registrations
            .borrow()
            .iter()
            .filter_map(|registration| match registration {
                Registration::Event(name, handler) if name == event => Some(Rc::clone(handler)),
                _ => None,
            })
            .collect()
    }

    /// The names of the registrations in registration order, each with its kind.
    pub fn registered(&self) -> Vec<String> {
        self.registrations
            .borrow()
            .iter()
            .map(|registration| match registration {
                Registration::Event(name, _) => format!("event {name}"),
                Registration::Command(name, _) => format!("command {name}"),
                Registration::Tool(name, _) => format!("tool {name}"),
                Registration::Shortcut(name, _) => format!("shortcut {name}"),
                Registration::Renderer(name, _) => format!("renderer {name}"),
            })
            .collect()
    }

    /// The callbacks of the tool registered under `name`.
    pub fn tool(&self, name: &str) -> Option<(Option<PrepareArguments>, ToolExecute)> {
        self.registrations
            .borrow()
            .iter()
            .find_map(|registration| match registration {
                Registration::Tool(registered, tool) if registered == name => {
                    Some((tool.prepare_arguments.clone(), Rc::clone(&tool.execute)))
                }
                _ => None,
            })
    }

    /// The metadata of the tool registered under `name`.
    pub fn tool_metadata(&self, name: &str) -> Option<maestro_extensions_wasm::ToolMetadata> {
        self.registrations
            .borrow()
            .iter()
            .find_map(|registration| match registration {
                Registration::Tool(registered, tool) if registered == name => {
                    Some(tool.metadata.clone())
                }
                _ => None,
            })
    }

    /// An emitter that logs what a synchronous callback emits.
    pub fn emitter(&self) -> Emitter {
        Emitter(self.log.clone())
    }

    /// The handler registered for a command name.
    pub fn command_handler(&self, command: &str) -> Option<CommandHandler> {
        self.command(command).map(|options| options.handler)
    }

    /// The registration of a command, with its handler and completions.
    pub fn command(&self, command: &str) -> Option<CommandOptions> {
        self.registrations
            .borrow()
            .iter()
            .find_map(|registration| match registration {
                Registration::Command(name, options) if name == command => Some(CommandOptions {
                    description: options.description.clone(),
                    get_argument_completions: options.get_argument_completions.clone(),
                    handler: Rc::clone(&options.handler),
                }),
                _ => None,
            })
    }

    /// The registration of a shortcut.
    pub fn shortcut(&self, shortcut: &str) -> Option<ShortcutOptions> {
        self.registrations
            .borrow()
            .iter()
            .find_map(|registration| match registration {
                Registration::Shortcut(name, options) if name == shortcut => {
                    Some(ShortcutOptions {
                        description: options.description.clone(),
                        handler: Rc::clone(&options.handler),
                    })
                }
                _ => None,
            })
    }

    /// The renderer registered for a custom message type.
    pub fn renderer(&self, custom_type: &str) -> Option<MessageRenderer> {
        self.registrations
            .borrow()
            .iter()
            .find_map(|registration| match registration {
                Registration::Renderer(name, renderer) if name == custom_type => {
                    Some(Rc::clone(renderer))
                }
                _ => None,
            })
    }

    /// Sets the value the command line supplied for a flag.
    pub fn supply_flag(&self, name: &str, value: &FlagValue) {
        for (flag, _, current) in self.flags.borrow_mut().iter_mut() {
            if flag == name {
                *current = Some(value.clone());
            }
        }
    }

    /// Delivers `data` on a bus channel as another extension would, running the tails.
    pub async fn deliver(&self, channel: &str, data: Value) -> ExtensionResult<()> {
        for tail in self.bus.deliver(channel, &data) {
            tail.await?;
        }
        Ok(())
    }

    /// An ordinary context over a fresh live session working in `/work`.
    pub fn context() -> ExtensionContext {
        Self::context_with(&Script::default())
    }

    /// An ordinary context whose operations answer from `script`.
    pub fn context_with(script: &Script) -> ExtensionContext {
        ExtensionContext::new(Rc::new(Ordinary {
            session: Session::new("/work"),
            script: script.clone(),
        }))
    }

    /// A command context over a fresh live session working in `/work`.
    pub fn command_context(&self) -> ExtensionCommandContext {
        let command = Command {
            session: Session::new("/work"),
            log: self.log.clone(),
            script: self.script.clone(),
            hold: Rc::clone(&self.hold),
        };
        ExtensionCommandContext::new(Rc::new(command))
    }

    /// Drops every registration in registration order.
    pub fn release_all(&self) {
        let registrations = std::mem::take(&mut *self.registrations.borrow_mut());
        for registration in registrations {
            drop(registration);
        }
    }

    /// Runs an event handler against an event the caller keeps.
    pub async fn dispatch(
        &self,
        event: &str,
        edited: &mut ExtensionEvent,
    ) -> ExtensionResult<Option<ExtensionEventResult>> {
        let handler = self
            .handler(event)
            .ok_or_else(|| format!("no handler for {event}"))?;
        handler(edited, Self::context()).await
    }
}

impl ExtensionHost for ControlledHost {
    fn on(&self, event: &str, handler: ExtensionHandler) -> ExtensionResult<()> {
        if event == REJECTED {
            self.log.push(format!("reject event {event}"));
            return Err(format!("registration rejected: {event}"));
        }
        self.log.push(format!("register event {event}"));
        self.registrations
            .borrow_mut()
            .push(Registration::Event(event.to_owned(), handler));
        Ok(())
    }

    fn register_tool(&self, tool: ToolDefinition) -> ExtensionResult<()> {
        let name = tool.metadata.name.clone();
        self.log.push(format!(
            "register tool {name} prepare={}",
            tool.prepare_arguments.is_some()
        ));
        self.registrations
            .borrow_mut()
            .push(Registration::Tool(name, tool));
        Ok(())
    }

    fn register_command(&self, name: &str, options: CommandOptions) -> ExtensionResult<()> {
        self.log.push(format!("register command {name}"));
        self.registrations
            .borrow_mut()
            .push(Registration::Command(name.to_owned(), options));
        Ok(())
    }

    fn register_shortcut(&self, shortcut: &str, options: ShortcutOptions) -> ExtensionResult<()> {
        self.log.push(format!("register shortcut {shortcut}"));
        self.registrations
            .borrow_mut()
            .push(Registration::Shortcut(shortcut.to_owned(), options));
        Ok(())
    }

    fn register_flag(&self, name: &str, options: FlagOptions) -> ExtensionResult<()> {
        self.log.push(format!("register flag {name}"));
        let current = options.default.clone();
        self.flags
            .borrow_mut()
            .push((name.to_owned(), options, current));
        Ok(())
    }

    fn get_flag(&self, name: &str) -> ExtensionResult<Option<FlagValue>> {
        let flags = self.flags.borrow();
        Ok(flags
            .iter()
            .find(|(flag, _, _)| flag == name)
            .and_then(|(_, _, value)| value.clone()))
    }

    fn register_message_renderer(
        &self,
        custom_type: &str,
        renderer: MessageRenderer,
    ) -> ExtensionResult<()> {
        self.log.push(format!("register renderer {custom_type}"));
        self.registrations
            .borrow_mut()
            .push(Registration::Renderer(custom_type.to_owned(), renderer));
        Ok(())
    }

    fn append_entry(&self, custom_type: &str, data: Option<Value>) -> ExtensionResult<()> {
        let data = data.map_or_else(|| "none".to_owned(), |data| data.to_string());
        self.log.push(format!("entry {custom_type} {data}"));
        Ok(())
    }

    scripted! { script;
        fn send_message(message: maestro_extensions_wasm::CustomMessageInput, options: Option<maestro_extensions_wasm::SendMessageOptions>) -> ();
        fn send_user_message(content: maestro_extensions_wasm::UserContent, options: Option<maestro_extensions_wasm::SendUserMessageOptions>) -> ();
        fn set_session_name(name: &str) -> ();
        fn get_session_name() -> Option<String>;
        fn set_label(entry_id: &str, label: Option<&str>) -> ();
        fn get_active_tools() -> Vec<String>;
        fn get_all_tools() -> Vec<maestro_extensions_wasm::ToolInfo>;
        fn set_active_tools(names: &[String]) -> ();
        fn get_commands() -> Vec<maestro_extensions_wasm::SlashCommandInfo>;
        fn get_thinking_level() -> maestro_extensions_wasm::ThinkingLevel;
        fn set_thinking_level(level: maestro_extensions_wasm::ThinkingLevel) -> ();
    }

    fn set_model(&self, model: Model) -> ExtensionFuture<'_, bool> {
        Box::pin(async move {
            self.script
                .answer("set_model", &[format!("{:?}", model.id)])
        })
    }

    fn events(&self) -> EventBus {
        EventBus::new(Rc::new(SharedBus(Rc::clone(&self.bus))))
    }
}
