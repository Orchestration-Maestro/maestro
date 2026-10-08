//! Controlled adapter: runs author code natively with registrations kept in memory.
//!
//! It implements the same ports as the component adapter, so one author function
//! runs through either. Tests drive it through the same dispatch functions the
//! component adapter's exports use.

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use serde_json::Value;

use crate::api::{ExtensionFuture, ExtensionHost, ExtensionResult, InputHandler};
use crate::bindings::maestro::extension::types::SessionChangeResult;
use crate::component::MessageRenderer;
use crate::context::{
    AbortSignal, CommandContextPort, CommandHandler, ContextPort, ExtensionCommandContext,
    ExtensionContext, NewSessionCommandOptions, ReplacedSessionContext, ReplacedSessionContextPort,
    SignalPort,
};
use crate::tool::{AgentToolResult, AgentToolUpdateCallback, ToolDefinition};

/// Message a stale context reports.
pub const STALE_CONTEXT: &str = "This context is stale after session replacement.";

/// Future a test supplies to hold a new session open.
pub type Gate = Box<dyn FnOnce() -> Pin<Box<dyn Future<Output = ()>>>>;

/// Session state a context is bound to.
pub struct Session {
    /// Working directory reported while the session is live.
    cwd: String,
    /// Set once the session has been replaced.
    stale: Cell<bool>,
}

impl Session {
    /// A live session working in `cwd`.
    #[must_use]
    pub fn new(cwd: &str) -> Rc<Self> {
        Rc::new(Self {
            cwd: cwd.to_owned(),
            stale: Cell::new(false),
        })
    }

    /// The directory, or the stale-context message.
    fn cwd(&self) -> ExtensionResult<String> {
        if self.stale.get() {
            Err(STALE_CONTEXT.to_owned())
        } else {
            Ok(self.cwd.clone())
        }
    }
}

/// A cancellation flag a test flips.
pub struct Flag(pub Cell<bool>);

impl SignalPort for Flag {
    fn aborted(&self) -> bool {
        self.0.get()
    }
}

/// Ordinary context over a session.
struct Context {
    /// Session the context is bound to.
    session: Rc<Session>,
    /// Signal this context hands out.
    signal: Option<Rc<Flag>>,
}

impl ContextPort for Context {
    fn cwd(&self) -> ExtensionResult<String> {
        self.session.cwd()
    }

    fn signal(&self) -> ExtensionResult<Option<AbortSignal>> {
        Ok(self
            .signal
            .as_ref()
            .map(|flag| AbortSignal::new(Rc::clone(flag) as Rc<dyn SignalPort>)))
    }
}

/// Command context over a session.
struct Command {
    /// Ordinary capabilities of the command context.
    context: Context,
    /// Host that records the operations.
    host: Rc<ControlledHost>,
}

impl ContextPort for Command {
    fn cwd(&self) -> ExtensionResult<String> {
        self.context.cwd()
    }

    fn signal(&self) -> ExtensionResult<Option<AbortSignal>> {
        self.context.signal()
    }
}

impl CommandContextPort for Command {
    fn new_session(
        &self,
        options: Option<NewSessionCommandOptions>,
    ) -> ExtensionFuture<'_, SessionChangeResult> {
        Box::pin(async move {
            let NewSessionCommandOptions {
                parent_session,
                with_session,
            } = options.unwrap_or_default();
            let parent = parent_session.as_deref().unwrap_or("none");
            self.host.log(format!("new-session start parent={parent}"));
            let gate = self.host.gate.borrow_mut().take();
            if let Some(gate) = gate {
                gate().await;
            }
            let replacement = Session::new("/replacement");
            if let Some(with_session) = with_session {
                let replaced = Rc::new(Replaced {
                    session: replacement,
                    host: Rc::clone(&self.host),
                });
                with_session(ReplacedSessionContext::new(replaced)).await?;
            }
            self.context.session.stale.set(true);
            self.host.log("new-session done cancelled=false".to_owned());
            Ok(SessionChangeResult { cancelled: false })
        })
    }
}

/// Context of the session a command created.
struct Replaced {
    /// The replacement session.
    session: Rc<Session>,
    /// Host that records the operations.
    host: Rc<ControlledHost>,
}

impl ReplacedSessionContextPort for Replaced {
    fn command(&self) -> Rc<dyn CommandContextPort> {
        Rc::new(Command {
            context: Context {
                session: Rc::clone(&self.session),
                signal: None,
            },
            host: Rc::clone(&self.host),
        })
    }

    fn send_user_message(&self, text: &str) -> ExtensionFuture<'_, ()> {
        self.host
            .log(format!("user-message {} {text}", self.session.cwd));
        Box::pin(async { Ok(()) })
    }
}

/// In-memory host: records registrations and a transcript, runs nothing on its own.
#[derive(Default)]
pub struct ControlledHost {
    /// Lines the host observed, in order.
    transcript: RefCell<Vec<String>>,
    /// Registered input handlers.
    inputs: RefCell<Vec<InputHandler>>,
    /// Registered tools.
    tools: RefCell<Vec<ToolDefinition>>,
    /// Registered commands by name.
    commands: RefCell<Vec<(String, CommandHandler)>>,
    /// Registered renderers by custom type.
    renderers: RefCell<Vec<(String, MessageRenderer)>>,
    /// Holds the next new session open.
    gate: RefCell<Option<Gate>>,
}

impl ControlledHost {
    /// An empty host.
    #[must_use]
    pub fn new() -> Rc<Self> {
        Rc::new(Self::default())
    }

    /// Records one observed line.
    fn log(&self, line: String) {
        self.transcript.borrow_mut().push(line);
    }

    /// Lines the host observed, in order.
    #[must_use]
    pub fn transcript(&self) -> Vec<String> {
        self.transcript.borrow().clone()
    }

    /// Appends a line the driving test observed.
    pub fn note(&self, line: String) {
        self.log(line);
    }

    /// Holds the next new session open until `gate` completes.
    pub fn hold_next_new_session(&self, gate: Gate) {
        *self.gate.borrow_mut() = Some(gate);
    }

    /// The first registered input handler.
    #[must_use]
    pub fn input_handler(&self) -> Option<InputHandler> {
        self.inputs.borrow().first().cloned()
    }

    /// A registered tool by name.
    #[must_use]
    pub fn tool(&self, name: &str) -> Option<ToolDefinition> {
        self.tools
            .borrow()
            .iter()
            .find(|tool| tool.metadata.name == name)
            .cloned()
    }

    /// A registered command handler by name.
    #[must_use]
    pub fn command(&self, name: &str) -> Option<CommandHandler> {
        self.commands
            .borrow()
            .iter()
            .find(|(command, _)| command == name)
            .map(|(_, handler)| Rc::clone(handler))
    }

    /// A registered renderer by custom type.
    #[must_use]
    pub fn renderer(&self, custom_type: &str) -> Option<MessageRenderer> {
        self.renderers
            .borrow()
            .iter()
            .find(|(kind, _)| kind == custom_type)
            .map(|(_, renderer)| Rc::clone(renderer))
    }

    /// An ordinary context over `session`, optionally carrying a signal.
    #[must_use]
    pub fn context(&self, session: &Rc<Session>, signal: Option<&Rc<Flag>>) -> ExtensionContext {
        ExtensionContext::new(Rc::new(Context {
            session: Rc::clone(session),
            signal: signal.cloned(),
        }))
    }

    /// A command context over `session`.
    #[must_use]
    pub fn command_context(self: &Rc<Self>, session: &Rc<Session>) -> ExtensionCommandContext {
        let context = Context {
            session: Rc::clone(session),
            signal: None,
        };
        ExtensionCommandContext::new(Rc::new(Command {
            context,
            host: Rc::clone(self),
        }))
    }

    /// A progress callback that records partial results.
    #[must_use]
    pub fn update_callback(self: &Rc<Self>, call_id: &str) -> AgentToolUpdateCallback {
        let host = Rc::clone(self);
        let call_id = call_id.to_owned();
        Rc::new(move |partial: AgentToolResult| {
            host.log(format!(
                "tool-update {call_id} {}",
                partial.content.join(",")
            ));
            Ok(())
        })
    }

    /// Drops every registered callback in registration order.
    pub fn release_all(&self) {
        let inputs = std::mem::take(&mut *self.inputs.borrow_mut());
        drop(inputs);
        let tools = std::mem::take(&mut *self.tools.borrow_mut());
        drop(tools);
        let commands = std::mem::take(&mut *self.commands.borrow_mut());
        drop(commands);
        let renderers = std::mem::take(&mut *self.renderers.borrow_mut());
        drop(renderers);
    }
}

impl ExtensionHost for ControlledHost {
    fn on_input(&self, handler: InputHandler) -> ExtensionResult<()> {
        self.log("register input".to_owned());
        self.inputs.borrow_mut().push(handler);
        Ok(())
    }

    fn register_tool(&self, tool: ToolDefinition) -> ExtensionResult<()> {
        self.log(format!(
            "register tool {} prepare={}",
            tool.metadata.name,
            tool.prepare_arguments.is_some()
        ));
        self.tools.borrow_mut().push(tool);
        Ok(())
    }

    fn register_command(&self, name: &str, handler: CommandHandler) -> ExtensionResult<()> {
        self.log(format!("register command {name}"));
        self.commands.borrow_mut().push((name.to_owned(), handler));
        Ok(())
    }

    fn register_message_renderer(
        &self,
        custom_type: &str,
        renderer: MessageRenderer,
    ) -> ExtensionResult<()> {
        self.log(format!("register renderer {custom_type}"));
        self.renderers
            .borrow_mut()
            .push((custom_type.to_owned(), renderer));
        Ok(())
    }

    fn append_entry(&self, custom_type: &str, data: Option<&Value>) -> ExtensionResult<()> {
        let data = data.map_or_else(|| "none".to_owned(), Value::to_string);
        self.log(format!("entry {custom_type} {data}"));
        Ok(())
    }
}
