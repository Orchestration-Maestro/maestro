//! The controlled adapter: an in-process host that implements the wire imports, so the
//! component adapter's own functions run natively against it, and that records what a real
//! host would observe.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::cell::RefCell;
use std::rc::Rc;

use super::observed::{Observed, Session, pause};
use crate::bindings::maestro::extension::session::{NewSessionCommandData, SessionChangeResult};
use crate::component_adapter::{Exports, Imports};
use crate::types::{ExtensionFuture, ExtensionResult};

/// A cancellation flag the test controls.
#[derive(Clone, Default)]
pub struct Flag(std::sync::Arc<std::sync::atomic::AtomicBool>);

impl Flag {
    /// Cancels the flag.
    pub fn abort(&self) {
        self.0.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Counts one resource lent to the extension until the extension drops it.
pub(super) struct Lease(Rc<RefCell<Observed>>);

impl Lease {
    /// Lends a resource.
    pub(super) fn new(observed: &Rc<RefCell<Observed>>) -> Self {
        observed.borrow_mut().lend();
        Self(Rc::clone(observed))
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        self.0.borrow_mut().reclaim();
    }
}

/// Owner handle of a callback identity; the host sees it dropped.
pub struct Callback {
    /// Key of the identity.
    id: u32,
    /// What the host observed.
    observed: Rc<RefCell<Observed>>,
}

impl Drop for Callback {
    fn drop(&mut self) {
        self.observed.borrow_mut().drop_identity(self.id);
    }
}

/// Host cancellation flag lent to the extension.
pub struct Signal {
    /// The flag the test controls.
    flag: Flag,
    /// Marks the resource as lent.
    _lease: Lease,
}

/// Progress resource lent to tool execution.
pub struct Update {
    /// Observations shared with the host.
    observed: Rc<RefCell<Observed>>,
    /// Counts its owned lifetime.
    _lease: Lease,
}

/// Ordinary context lent to the extension.
pub struct Context {
    /// Captured model supplier.
    model: super::reader_host::Reader,
    /// The session the context is bound to.
    session: Session,
    /// Marks the resource as lent.
    _lease: Lease,
}

/// Command context lent to the extension.
pub struct CommandContext {
    /// Captured model supplier.
    model: super::reader_host::Reader,
    /// The session the context is bound to.
    session: Session,
    /// Marks the resource as lent.
    _lease: Lease,
}

/// Replacement context lent to the extension.
pub struct ReplacedContext {
    /// The session the context is bound to.
    session: Session,
    /// Marks the resource as lent.
    _lease: Lease,
}

/// Retained reader owner counted until its final facade alias is removed.
pub struct Reader {
    /// Retained session state.
    data: super::reader_host::Reader,
    /// Resource ownership witness.
    _lease: Lease,
}
/// Retained node owner independent of its parent.
pub struct Node {
    /// Captured node.
    data: super::reader_host::Node,
    /// Resource ownership witness.
    _lease: Lease,
}

/// The controlled host: records registrations and serves contexts from memory.
#[derive(Clone, Default)]
pub struct Controlled {
    /// What the host observed.
    observed: Rc<RefCell<Observed>>,
}

impl Controlled {
    /// What the host observed so far.
    pub fn observed(&self) -> std::cell::Ref<'_, Observed> {
        self.observed.borrow()
    }

    /// Changes what the host observed, such as the hold of the next wait for idle.
    pub fn observe(&self) -> std::cell::RefMut<'_, Observed> {
        self.observed.borrow_mut()
    }

    /// A cancellation signal reading `flag`.
    pub fn signal(&self, flag: &Flag) -> Signal {
        Signal {
            flag: flag.clone(),
            _lease: Lease::new(&self.observed),
        }
    }

    /// An ordinary context in `cwd`.
    pub fn context(&self, cwd: &str) -> Context {
        let model = self.observed.borrow().catalog.model.clone();
        Context {
            model,
            session: Session::new(cwd),
            _lease: Lease::new(&self.observed),
        }
    }

    /// A progress resource sharing this host's observations.
    pub fn update(&self) -> Update {
        Update {
            observed: Rc::clone(&self.observed),
            _lease: Lease::new(&self.observed),
        }
    }

    /// A command context in `cwd`.
    pub fn command_context(&self, cwd: &str) -> CommandContext {
        let model = self.observed.borrow().catalog.model.clone();
        CommandContext {
            model,
            session: Session::new(cwd),
            _lease: Lease::new(&self.observed),
        }
    }

    /// A replacement context in `cwd`.
    pub fn replaced_context(&self, cwd: &str) -> ReplacedContext {
        ReplacedContext {
            session: Session::new(cwd),
            _lease: Lease::new(&self.observed),
        }
    }

    /// Runs the continuation announced for an operation against a fresh replacement context.
    async fn continue_with(&self, callback: &Callback, cwd: &str) -> ExtensionResult<()> {
        Exports::new(self.clone())
            .invoke_with_session(callback.id, self.replaced_context(cwd))
            .await
    }
}

/// Retained opaque UI owner with a counted lifetime.
pub struct Ui {
    /// Counted owner.
    _lease: Lease,
}
impl Imports for Controlled {
    type Subscription = super::bus_host::Subscription;
    type Emitter = super::bus_host::Emitter;
    fn event_on(
        &self,
        channel: &str,
        handler: &Self::Callback,
    ) -> ExtensionResult<Self::Subscription> {
        let subscription = self.observed.borrow_mut().bus.on(channel, handler.id)?;
        self.observed
            .borrow_mut()
            .on(&format!("bus {}", handler.id), handler.id)?;
        Ok(subscription)
    }
    fn unsubscribe(&self, subscription: &Self::Subscription) -> ExtensionResult<()> {
        self.observed.borrow_mut().bus.unsubscribe(subscription)
    }
    fn event_emit(&self, channel: String, data: String) -> ExtensionFuture<'_, ()> {
        Box::pin(async move { self.callback_emit(&super::bus_host::Emitter, &channel, &data) })
    }
    fn callback_emit(
        &self,
        emitter: &Self::Emitter,
        channel: &str,
        data: &str,
    ) -> ExtensionResult<()> {
        let handlers = self.observed.borrow_mut().bus.emit(channel, data)?;
        for handler in handlers {
            Exports::new(self.clone()).invoke_listener(handler, data, emitter)?;
        }
        Ok(())
    }
    fn start_event_tail(
        &self,
        _emitter: &Self::Emitter,
        tail: &Self::Callback,
    ) -> ExtensionResult<()> {
        self.observed.borrow_mut().bus.tail(tail.id)
    }

    type Ui = Ui;
    type Registry = Reader;
    type Reader = Reader;
    type Node = Node;
    type Update = Update;
    type Callback = Callback;
    type Signal = Signal;
    type Context = Context;
    type CommandContext = CommandContext;
    type ReplacedContext = ReplacedContext;

    fn session_manager(&self, _context: &Context) -> ExtensionResult<Reader> {
        let data = self.observed.borrow().readers.acquire()?;
        Ok(Reader {
            data,
            _lease: Lease::new(&self.observed),
        })
    }
    fn command_session_manager(&self, _context: &CommandContext) -> ExtensionResult<Reader> {
        let data = self.observed.borrow().readers.acquire()?;
        Ok(Reader {
            data,
            _lease: Lease::new(&self.observed),
        })
    }
    fn reader_get_cwd(&self, resource: &Reader) -> ExtensionResult<String> {
        resource.data.string("getCwd")
    }
    fn reader_get_session_dir(&self, resource: &Reader) -> ExtensionResult<String> {
        resource.data.string("getSessionDir")
    }
    fn reader_get_session_id(&self, resource: &Reader) -> ExtensionResult<String> {
        resource.data.string("getSessionId")
    }
    fn reader_get_session_file(&self, resource: &Reader) -> ExtensionResult<Option<String>> {
        resource.data.optional("getSessionFile", &[])
    }
    fn reader_get_leaf_id(&self, resource: &Reader) -> ExtensionResult<Option<String>> {
        resource.data.optional("getLeafId", &[])
    }
    fn reader_get_leaf_entry(&self, resource: &Reader) -> ExtensionResult<Option<String>> {
        resource.data.record("getLeafEntry", &[])
    }
    fn reader_get_entry(&self, resource: &Reader, id: &str) -> ExtensionResult<Option<String>> {
        resource.data.record(
            "getEntry",
            &serde_json::json!([id]).as_array().unwrap().clone(),
        )
    }
    fn reader_get_label(&self, resource: &Reader, id: &str) -> ExtensionResult<Option<String>> {
        resource.data.optional(
            "getLabel",
            &serde_json::json!([id]).as_array().unwrap().clone(),
        )
    }
    fn reader_get_branch(
        &self,
        resource: &Reader,
        from_id: Option<&str>,
    ) -> ExtensionResult<String> {
        resource
            .data
            .record(
                "getBranch",
                &serde_json::json!([from_id]).as_array().unwrap().clone(),
            )?
            .ok_or_else(|| "missing list".into())
    }
    fn reader_get_header(&self, resource: &Reader) -> ExtensionResult<Option<String>> {
        resource.data.record("getHeader", &[])
    }
    fn reader_get_entries(&self, resource: &Reader) -> ExtensionResult<String> {
        resource
            .data
            .record("getEntries", &[])?
            .ok_or_else(|| "missing list".into())
    }
    fn reader_get_tree(&self, resource: &Reader) -> ExtensionResult<Vec<Self::Node>> {
        resource.data.tree().map(|nodes| {
            nodes
                .into_iter()
                .map(|data| Node {
                    data,
                    _lease: Lease::new(&self.observed),
                })
                .collect()
        })
    }
    fn reader_get_session_name(&self, resource: &Reader) -> ExtensionResult<Option<String>> {
        resource.data.optional("getSessionName", &[])
    }
    fn node_entry(&self, resource: &Node) -> ExtensionResult<String> {
        resource.data.entry()
    }
    fn node_children(&self, resource: &Node) -> ExtensionResult<Vec<Self::Node>> {
        resource.data.children().map(|nodes| {
            nodes
                .into_iter()
                .map(|data| Node {
                    data,
                    _lease: Lease::new(&self.observed),
                })
                .collect()
        })
    }
    fn node_label(&self, resource: &Node) -> ExtensionResult<Option<String>> {
        resource.data.label()
    }
    fn node_label_timestamp(&self, resource: &Node) -> ExtensionResult<Option<String>> {
        resource.data.label_timestamp()
    }

    fn ui(&self, context: &Context) -> ExtensionResult<Ui> {
        let _ = context;
        self.observed.borrow().catalog.check()?;
        Ok(Ui {
            _lease: Lease::new(&self.observed),
        })
    }
    fn has_ui(&self, context: &Context) -> ExtensionResult<bool> {
        let _ = context;
        self.observed.borrow().catalog.check()?;
        Ok(self.observed.borrow().catalog.data.has_ui)
    }
    fn model_registry(&self, context: &Context) -> ExtensionResult<Reader> {
        let _ = context;
        self.observed.borrow().catalog.check()?;
        let data = self.observed.borrow().catalog.registry.clone();
        Ok(Reader {
            data,
            _lease: Lease::new(&self.observed),
        })
    }
    fn model(&self, context: &Context) -> ExtensionResult<Option<String>> {
        let _ = context;
        self.observed.borrow().catalog.check()?;
        self.observed
            .borrow()
            .catalog
            .selected_model(&context.model)
    }
    fn is_idle(&self, context: &Context) -> ExtensionResult<bool> {
        let _ = context;
        self.observed.borrow().catalog.check()?;
        Ok(self.observed.borrow().catalog.data.idle)
    }
    fn signal(&self, context: &Context) -> ExtensionResult<Option<Signal>> {
        let _ = context;
        self.observed.borrow().catalog.check()?;
        let flag = self.observed.borrow().catalog.flag.clone();
        Ok(flag.map(|flag| Signal {
            flag: Flag(flag),
            _lease: Lease::new(&self.observed),
        }))
    }
    fn abort(&self, context: &Context) -> ExtensionResult<()> {
        let _ = context;
        self.observed.borrow().catalog.check()?;
        self.observed.borrow_mut().log("context abort");
        Ok(())
    }
    fn has_pending_messages(&self, context: &Context) -> ExtensionResult<bool> {
        let _ = context;
        self.observed.borrow().catalog.check()?;
        Ok(self.observed.borrow().catalog.data.queued)
    }
    fn shutdown(&self, context: &Context) -> ExtensionResult<()> {
        let _ = context;
        self.observed.borrow().catalog.check()?;
        self.observed.borrow_mut().log("context shutdown");
        Ok(())
    }
    fn get_context_usage(&self, context: &Context) -> ExtensionResult<Option<crate::ContextUsage>> {
        let _ = context;
        self.observed.borrow().catalog.check()?;
        Ok(self
            .observed
            .borrow()
            .catalog
            .data
            .usage
            .as_ref()
            .map(|u| crate::ContextUsage {
                tokens: u.tokens,
                context_window: u.context_window,
                percent: u.percent,
            }))
    }
    fn get_system_prompt(&self, context: &Context) -> ExtensionResult<String> {
        let _ = context;
        self.observed.borrow().catalog.check()?;
        Ok(self.observed.borrow().catalog.data.prompt.clone())
    }
    fn command_ui(&self, context: &CommandContext) -> ExtensionResult<Ui> {
        let _ = context;
        self.observed.borrow().catalog.check()?;
        Ok(Ui {
            _lease: Lease::new(&self.observed),
        })
    }
    fn command_has_ui(&self, context: &CommandContext) -> ExtensionResult<bool> {
        let _ = context;
        self.observed.borrow().catalog.check()?;
        Ok(self.observed.borrow().catalog.data.has_ui)
    }
    fn command_model_registry(&self, context: &CommandContext) -> ExtensionResult<Reader> {
        let _ = context;
        self.observed.borrow().catalog.check()?;
        let data = self.observed.borrow().catalog.registry.clone();
        Ok(Reader {
            data,
            _lease: Lease::new(&self.observed),
        })
    }
    fn command_model(&self, context: &CommandContext) -> ExtensionResult<Option<String>> {
        let _ = context;
        self.observed.borrow().catalog.check()?;
        self.observed
            .borrow()
            .catalog
            .selected_model(&context.model)
    }
    fn command_is_idle(&self, context: &CommandContext) -> ExtensionResult<bool> {
        let _ = context;
        self.observed.borrow().catalog.check()?;
        Ok(self.observed.borrow().catalog.data.idle)
    }
    fn command_signal(&self, context: &CommandContext) -> ExtensionResult<Option<Signal>> {
        let _ = context;
        self.observed.borrow().catalog.check()?;
        let flag = self.observed.borrow().catalog.flag.clone();
        Ok(flag.map(|flag| Signal {
            flag: Flag(flag),
            _lease: Lease::new(&self.observed),
        }))
    }
    fn command_abort(&self, context: &CommandContext) -> ExtensionResult<()> {
        let _ = context;
        self.observed.borrow().catalog.check()?;
        self.observed.borrow_mut().log("context abort");
        Ok(())
    }
    fn command_has_pending_messages(&self, context: &CommandContext) -> ExtensionResult<bool> {
        let _ = context;
        self.observed.borrow().catalog.check()?;
        Ok(self.observed.borrow().catalog.data.queued)
    }
    fn command_shutdown(&self, context: &CommandContext) -> ExtensionResult<()> {
        let _ = context;
        self.observed.borrow().catalog.check()?;
        self.observed.borrow_mut().log("context shutdown");
        Ok(())
    }
    fn command_get_context_usage(
        &self,
        context: &CommandContext,
    ) -> ExtensionResult<Option<crate::ContextUsage>> {
        let _ = context;
        self.observed.borrow().catalog.check()?;
        Ok(self
            .observed
            .borrow()
            .catalog
            .data
            .usage
            .as_ref()
            .map(|u| crate::ContextUsage {
                tokens: u.tokens,
                context_window: u.context_window,
                percent: u.percent,
            }))
    }
    fn command_get_system_prompt(&self, context: &CommandContext) -> ExtensionResult<String> {
        let _ = context;
        self.observed.borrow().catalog.check()?;
        Ok(self.observed.borrow().catalog.data.prompt.clone())
    }
    fn registry_get_all(&self, resource: &Reader) -> ExtensionResult<String> {
        resource
            .data
            .record("getAll", &[])?
            .ok_or("missing list".into())
    }
    fn registry_get_available(&self, resource: &Reader) -> ExtensionResult<String> {
        resource
            .data
            .record("getAvailable", &[])?
            .ok_or("missing list".into())
    }
    fn registry_find(
        &self,
        resource: &Reader,
        provider: &str,
        model_id: &str,
    ) -> ExtensionResult<Option<String>> {
        resource.data.record(
            "find",
            &[serde_json::json!(provider), serde_json::json!(model_id)],
        )
    }
    fn registry_get_api_key_and_headers<'a>(
        &'a self,
        resource: &'a Reader,
        model: String,
    ) -> ExtensionFuture<'a, crate::ResolvedRequestAuth> {
        Box::pin(async move {
            let hold = {
                let mut o = self.observed.borrow_mut();
                o.log("auth started");
                o.take_hold()
            };
            pause(hold).await;
            let answer = super::catalog_host::auth(&resource.data, &model)
                .map(|value| super::catalog_host::resolved(&value));
            self.observed.borrow_mut().log("auth completed");
            answer
        })
    }

    fn new_callback(&self) -> (u32, Callback) {
        let id = self.observed.borrow_mut().next_identity();
        (
            id,
            Callback {
                id,
                observed: Rc::clone(&self.observed),
            },
        )
    }

    fn on(&self, event: &str, handler: &Callback) -> ExtensionResult<()> {
        self.observed.borrow_mut().on(event, handler.id)
    }

    fn register_command(
        &self,
        name: &str,
        _description: Option<&str>,
        handler: &Callback,
    ) -> ExtensionResult<()> {
        self.observed
            .borrow_mut()
            .register_command(name, handler.id);
        Ok(())
    }

    fn register_tool(
        &self,
        metadata: &str,
        prepare: Option<&Callback>,
        execute: &Callback,
    ) -> ExtensionResult<()> {
        self.observed
            .borrow_mut()
            .register_tool(metadata, prepare.map(|p| p.id), execute.id)
    }

    fn tool_update(&self, update: &Update, partial: &str) -> ExtensionResult<()> {
        let value: serde_json::Value = serde_json::from_str(partial).map_err(|e| e.to_string())?;
        update.observed.borrow_mut().updates.push(partial.into());
        if value["details"] == "fail update" {
            return Err("failed update: Ω".into());
        }
        Ok(())
    }

    fn append_entry(&self, custom_type: &str, data: Option<&str>) -> ExtensionResult<()> {
        self.observed.borrow_mut().entry(custom_type, data);
        Ok(())
    }

    fn aborted(&self, signal: &Signal) -> bool {
        signal.flag.0.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn cwd(&self, context: &Context) -> ExtensionResult<String> {
        context.session.cwd()
    }

    fn command_cwd(&self, context: &CommandContext) -> ExtensionResult<String> {
        context.session.cwd()
    }

    fn wait_for_idle<'a>(&'a self, context: &'a CommandContext) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            let cwd = &context.session.cwd;
            let hold = {
                let mut observed = self.observed.borrow_mut();
                observed.log(format!("wait-for-idle {cwd}"));
                observed.take_hold()
            };
            pause(hold).await;
            Ok(())
        })
    }

    fn new_session<'a>(
        &'a self,
        context: &'a CommandContext,
        data: NewSessionCommandData,
        with_session: Option<&'a Callback>,
    ) -> ExtensionFuture<'a, SessionChangeResult> {
        Box::pin(async move {
            let parent = data.parent_session.as_deref().unwrap_or("none");
            self.observed
                .borrow_mut()
                .log(format!("new-session start parent={parent}"));
            if self.observed.borrow_mut().take_rejection() {
                return Err("session rejected".to_owned());
            }
            if let Some(callback) = with_session {
                self.continue_with(callback, "/replacement").await?;
            }
            context.session.stale.set(true);
            self.observed
                .borrow_mut()
                .log("new-session done cancelled=false");
            Ok(SessionChangeResult { cancelled: false })
        })
    }

    fn command(&self, context: &ReplacedContext) -> CommandContext {
        self.command_context(&context.session.cwd)
    }
}
