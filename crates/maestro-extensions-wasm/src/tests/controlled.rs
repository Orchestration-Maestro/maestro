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
use super::setup_host::document;
use crate::bindings::maestro::extension::host::CompactionData;
use crate::bindings::maestro::extension::session::{NewSessionCommandData, SessionChangeResult};
use crate::component_adapter::{Dialog, Exports, Imports};
use crate::types::{ExtensionFuture, ExtensionResult};
use serde_json::json;

/// A cancellation flag the test controls.
#[derive(Clone, Default)]
pub struct Flag(pub(super) std::sync::Arc<std::sync::atomic::AtomicBool>);

impl Flag {
    /// Shared flag for controlled receiver cancellation.
    pub fn shared(&self) -> std::sync::Arc<std::sync::atomic::AtomicBool> {
        self.0.clone()
    }
    /// Stable key for the owned signal witness.
    pub fn key(&self) -> usize {
        std::sync::Arc::as_ptr(&self.0) as usize
    }
    /// Cancels the flag.
    pub fn abort(&self) {
        self.0.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

/// The bus channel that keeps the cancellation listeners of `flag`.
fn abort_channel(flag: &Flag) -> String {
    format!("abort {}", flag.key())
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
    pub(super) id: u32,
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
    lease: Lease,
}

impl Drop for Signal {
    fn drop(&mut self) {
        let key = std::sync::Arc::as_ptr(&self.flag.0) as usize;
        *self
            .lease
            .0
            .borrow_mut()
            .shell
            .signal_drops
            .entry(key)
            .or_default() += 1;
    }
}

/// Progress resource lent to tool execution.
pub struct Update {
    /// Observations shared with the host.
    observed: Rc<RefCell<Observed>>,
    /// Cancellation flag the host raises on the first recorded update.
    cancel_on_first: Option<Flag>,
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
    /// Operation-bound setup reader, when supplied.
    setup: Option<super::reader_host::Reader>,
    /// Captured model supplier.
    model: super::reader_host::Reader,
    /// The session the context is bound to.
    session: Session,
    /// Marks the resource as lent.
    _lease: Lease,
}

/// Replacement context lent to the extension.
pub struct ReplacedContext {
    /// Operation-bound setup reader.
    setup: Option<super::reader_host::Reader>,
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

/// Counted theme owner with its literal identity.
pub struct Theme {
    /// Supplied identity.
    pub(super) key: String,
    /// Shared observations.
    observed: Rc<RefCell<Observed>>,
    /// Owned resource count.
    _lease: Lease,
}
impl Drop for Theme {
    fn drop(&mut self) {
        self.observed
            .borrow_mut()
            .log(format!("theme-drop {}", self.key));
    }
}

/// The controlled host: records registrations and serves contexts from memory.
#[derive(Clone, Default)]
pub struct Controlled {
    /// What the host observed.
    pub(super) observed: Rc<RefCell<Observed>>,
    /// Components factory widgets built, by the handle their slots name.
    widgets: Rc<RefCell<std::collections::HashMap<u32, crate::ExtensionComponent>>>,
}

/// Terminal lent to a component factory, with a counted lifetime.
pub struct Tui {
    /// Counted owner.
    _lease: Lease,
}
impl Tui {
    /// Lends a counted terminal or overlay handle.
    pub(super) fn lent(observed: &Rc<RefCell<Observed>>) -> Self {
        Self {
            _lease: Lease::new(observed),
        }
    }
}

/// Retained placement and its counted host capability.
pub struct OverlayOptions {
    /// The shared record.
    pub(super) value: super::overlay_host::Options,
    /// The identity of the record's visibility check, shared by its owners.
    pub(super) visible: Rc<std::cell::Cell<Option<u32>>>,
    /// Counted resource lifetime.
    _lease: Lease,
}

/// Redraw capability of one tool row, with a counted lifetime.
pub struct Invalidator {
    /// The row it redraws.
    pub row: String,
    /// Counted owner.
    _lease: Lease,
}

/// Footer metadata lent to a footer factory, with a counted lifetime.
pub struct FooterData {
    /// Counted owner.
    _lease: Lease,
}

impl Controlled {
    /// Removes every widget under `key` from both placements, running each disposal before
    /// the component is released.
    fn take_widgets(&self, key: &str) -> ExtensionResult<()> {
        let removed: Vec<_> = {
            let mut observed = self.observed.borrow_mut();
            let (removed, kept) = std::mem::take(&mut observed.ui.widgets)
                .into_iter()
                .partition(|slot| slot.key == key);
            observed.ui.widgets = kept;
            removed
        };
        for slot in removed {
            let built = self.widgets.borrow_mut().remove(&slot.component);
            let dispose = built.as_ref().and_then(|built| built.dispose.clone());
            dispose.map_or(Ok(()), |dispose| self.dispose(&dispose))?;
            drop(built);
        }
        Ok(())
    }

    /// Builds the component a factory under `factory` makes with a lent terminal and theme,
    /// and keeps it under a fresh handle.
    fn build(&self, factory: &Callback, footer: bool) -> ExtensionResult<u32> {
        let lent = crate::component_adapter::factories::Lent {
            tui: Tui {
                _lease: Lease::new(&self.observed),
            },
            theme: self.theme("dark"),
            footer: footer.then(|| FooterData {
                _lease: Lease::new(&self.observed),
            }),
        };
        let emitter = super::bus_host::Emitter { key: 0 };
        let built =
            Exports::new(self.clone()).invoke_component_factory(factory.id, lent, &emitter)?;
        let handle = self.observed.borrow_mut().next_identity();
        self.widgets.borrow_mut().insert(handle, built);
        Ok(handle)
    }

    /// Keeps a component an authored widget callback returned, under a fresh identity.
    pub(super) fn keep_component(&self, component: crate::Component) -> u32 {
        let handle = self.observed.borrow_mut().next_identity();
        let kept = crate::ExtensionComponent {
            component,
            dispose: None,
            set_expanded: None,
            width: None,
        };
        self.widgets.borrow_mut().insert(handle, kept);
        handle
    }

    /// The component kept under `handle`.
    pub(super) fn kept_component(&self, handle: u32) -> Option<crate::Component> {
        let kept = self.widgets.borrow();
        kept.get(&handle).map(|kept| kept.component.clone())
    }

    /// The redraw capability of the tool row `row`.
    pub fn invalidator(&self, row: &str) -> Invalidator {
        Invalidator {
            row: row.to_owned(),
            _lease: Lease::new(&self.observed),
        }
    }

    /// Drops the custom footer, its handle and the factory that built it, as a host that
    /// discards them does.
    pub fn release_footer(&self) {
        let slot = self.observed.borrow_mut().ui.footer.custom.take();
        if let Some(slot) = slot {
            drop(self.widgets.borrow_mut().remove(&slot.component));
            self.observed.borrow_mut().drop_identity(slot.component);
        }
        let factory = self.observed.borrow_mut().ui.footer.factory.take();
        if let Some(factory) = factory {
            crate::component_adapter::release(factory);
        }
    }

    /// The component the custom footer shows.
    pub fn footer(&self) -> Option<crate::Component> {
        let slot = self
            .observed
            .borrow()
            .ui
            .footer
            .custom
            .as_ref()
            .map(|slot| slot.component)?;
        self.widgets
            .borrow()
            .get(&slot)
            .map(|built| built.component.clone())
    }

    /// The component the custom header shows.
    pub fn header(&self) -> Option<crate::Component> {
        let slot = self
            .observed
            .borrow()
            .ui
            .header
            .custom
            .as_ref()
            .map(|slot| slot.component)?;
        self.widgets
            .borrow()
            .get(&slot)
            .map(|built| built.component.clone())
    }

    /// Runs one widget disposal with a lent emitter.
    /// Runs an editor factory with the host's terminal, editor styles and keybindings, and
    /// shows what it built.
    fn build_editor_component(&self, factory: u32) -> ExtensionResult<()> {
        let roles = [
            "selectedPrefix",
            "selectedText",
            "description",
            "scrollInfo",
            "noMatch",
        ];
        let lent = crate::component_adapter::LentEditor {
            tui: Tui {
                _lease: Lease::new(&self.observed),
            },
            theme: (
                super::widget_controlled::Style("borderMuted".to_owned()),
                roles
                    .map(|role| super::widget_controlled::Style(role.to_owned()))
                    .into(),
            ),
            keybindings: super::widget_controlled::Keybindings(0),
        };
        let emitter = super::bus_host::Emitter { key: 0 };
        let outcome = Exports::new(self.clone()).invoke_editor_factory(factory, lent, &emitter)?;
        let (component, editor, custom) =
            crate::component_adapter::keep_editor(self, &outcome, |widget| widget.0)?;
        let slot = super::ui_host::EditorSlot {
            factory,
            component,
            editor,
            custom,
        };
        super::ui_host::show_editor(&mut self.observed.borrow_mut(), slot);
        Ok(())
    }

    fn dispose(&self, dispose: &crate::ComponentDisposer) -> ExtensionResult<()> {
        let emitter = super::bus_host::Emitter { key: 0 };
        crate::component_adapter::renderers::with_emitter(self, &emitter, |emitter| {
            dispose(emitter)
        })
    }

    /// The complete component the factory under `key` built.
    pub fn widget_built(&self, key: &str) -> Option<crate::ExtensionComponent> {
        let slot = self
            .observed
            .borrow()
            .ui
            .widgets
            .iter()
            .find(|slot| slot.key == key)?
            .component;
        self.widgets.borrow().get(&slot).cloned()
    }

    /// The component the factory widget under `key` shows.
    pub fn widget(&self, key: &str) -> Option<crate::Component> {
        let slot = self
            .observed
            .borrow()
            .ui
            .widgets
            .iter()
            .find(|slot| slot.key == key)
            .map(|slot| slot.component)?;
        self.widgets
            .borrow()
            .get(&slot)
            .map(|built| built.component.clone())
    }
}

impl Controlled {
    /// Lends a raw byte receiver with its own lifetime.
    pub fn bash_receiver(&self, id: usize) -> super::shell_host::NativeReceiver {
        super::shell_host::NativeReceiver {
            observed: Rc::clone(&self.observed),
            id,
            _lease: Lease::new(&self.observed),
        }
    }
    /// Lends an independently owned theme.
    pub fn theme(&self, key: &str) -> Theme {
        Theme {
            key: key.into(),
            observed: Rc::clone(&self.observed),
            _lease: Lease::new(&self.observed),
        }
    }
    /// What the host observed so far.
    pub fn observed(&self) -> std::cell::Ref<'_, Observed> {
        self.observed.borrow()
    }

    /// Changes what the host observed, such as the hold of the next wait for idle.
    pub fn observe(&self) -> std::cell::RefMut<'_, Observed> {
        self.observed.borrow_mut()
    }

    /// Owned writer fixture counted until its last alias is dropped.
    pub fn writer(&self, name: &str) -> Reader {
        Reader {
            data: super::setup_host::reader(name),
            _lease: Lease::new(&self.observed),
        }
    }

    /// Cancels `flag` and runs each listener subscribed to it once, in registration order,
    /// each with its own emitter; returns every listener's result.
    pub fn abort(&self, flag: &Flag) -> Vec<ExtensionResult<()>> {
        flag.abort();
        let channel = abort_channel(flag);
        let mut results = Vec::new();
        loop {
            let next = {
                let mut observed = self.observed.borrow_mut();
                let listeners = &mut observed.bus.listeners;
                let index = listeners.iter().position(|(name, _)| *name == channel);
                index.map(|index| listeners.remove(index).1)
            };
            let Some(id) = next else {
                return results;
            };
            let emitter = super::bus_host::Emitter { key: id as usize };
            results.push(Exports::new(self.clone()).invoke_abort(id, &emitter));
        }
    }

    /// A cancellation signal reading `flag`.
    pub fn signal(&self, flag: &Flag) -> Signal {
        Signal {
            flag: flag.clone(),
            lease: Lease::new(&self.observed),
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
            cancel_on_first: None,
            _lease: Lease::new(&self.observed),
        }
    }

    /// A progress resource that raises `flag` on its first recorded update.
    pub fn aborting_update(&self, flag: &Flag) -> Update {
        Update {
            observed: Rc::clone(&self.observed),
            cancel_on_first: Some(flag.clone()),
            _lease: Lease::new(&self.observed),
        }
    }

    /// A command context in `cwd`.
    pub fn command_context(&self, cwd: &str) -> CommandContext {
        let model = self.observed.borrow().catalog.model.clone();
        CommandContext {
            setup: None,
            model,
            session: Session::new(cwd),
            _lease: Lease::new(&self.observed),
        }
    }

    /// A replacement context in `cwd`.
    pub fn replaced_context(&self, cwd: &str) -> ReplacedContext {
        ReplacedContext {
            setup: None,
            session: Session::new(cwd),
            _lease: Lease::new(&self.observed),
        }
    }
}

/// Answers a text request on `theme` through the controlled host.
fn theme_text(
    observed: &Rc<RefCell<Observed>>,
    theme: &Theme,
    method: &str,
    operands: &serde_json::Value,
) -> ExtensionResult<String> {
    super::theme_host::text(&mut observed.borrow_mut(), &theme.key, method, operands)
}

/// Retained opaque UI owner with a counted lifetime.
pub struct Ui {
    /// Host identity of this acquisition.
    key: usize,
    /// Counted owner.
    _lease: Lease,
}

/// Records one retained UI request through the controlled host.
fn ui_call(
    observed: &Rc<RefCell<Observed>>,
    key: usize,
    method: &str,
    operands: &serde_json::Value,
) -> ExtensionResult<()> {
    super::ui_host::call(&mut observed.borrow_mut(), key, method, operands)
}
/// A selection result as success and its optional message.
fn theme_set_result(result: &serde_json::Value) -> crate::ThemeSetResult {
    crate::ThemeSetResult {
        success: result[0] == true,
        error: result[1].as_str().map(str::to_owned),
    }
}

impl Controlled {
    /// Records the compaction binding of `session` and forwards the request to the action host.
    fn compact_binding(
        &self,
        session: &Session,
        instructions: Option<&CompactionData>,
        complete: Option<&Callback>,
        error: Option<&Callback>,
    ) -> ExtensionResult<()> {
        self.observed.borrow().actions.check()?;
        let cwd = session.cwd()?;
        self.observed
            .borrow_mut()
            .log(format!("compact binding {cwd}"));
        super::action_host::compact(
            &mut self.observed.borrow_mut(),
            instructions.and_then(|data| data.custom_instructions.as_deref()),
            complete.map(|c| c.id),
            error.map(|c| c.id),
            instructions.is_some(),
        )
    }
    /// Acquires the user interface owner for the session working in `cwd`.
    fn acquire_ui(&self, cwd: &str) -> ExtensionResult<Ui> {
        self.observed.borrow().catalog.check()?;
        let key = self.observed.borrow_mut().ui.acquire(cwd);
        Ok(Ui {
            key,
            _lease: Lease::new(&self.observed),
        })
    }
    /// Whether the scripted catalog reports an interface.
    fn catalog_has_ui(&self) -> ExtensionResult<bool> {
        self.observed.borrow().catalog.check()?;
        Ok(self.observed.borrow().catalog.data.has_ui)
    }
    /// A reader over the scripted catalog's model registry.
    fn catalog_registry(&self) -> ExtensionResult<Reader> {
        self.observed.borrow().catalog.check()?;
        let data = self.observed.borrow().catalog.registry.clone();
        Ok(Reader {
            data,
            _lease: Lease::new(&self.observed),
        })
    }
    /// Records one theme request on `resource` and answers it from the controlled host.
    fn ui_themed(
        &self,
        resource: &Ui,
        method: &str,
        operand: &serde_json::Value,
    ) -> ExtensionResult<serde_json::Value> {
        super::ui_host::themed(
            &mut self.observed.borrow_mut(),
            resource.key,
            method,
            operand,
        )
    }
}

impl Controlled {
    /// The catalog's selection for the model supplier a context captured.
    fn catalog_model(&self, model: &super::reader_host::Reader) -> ExtensionResult<Option<String>> {
        self.observed.borrow().catalog.check()?;
        self.observed.borrow().catalog.selected_model(model)
    }
    /// Whether the scripted session is idle.
    fn catalog_idle(&self) -> ExtensionResult<bool> {
        self.observed.borrow().catalog.check()?;
        Ok(self.observed.borrow().catalog.data.idle)
    }
    /// The cancellation signal the scripted session lends, when it has one.
    fn catalog_signal(&self) -> ExtensionResult<Option<Signal>> {
        self.observed.borrow().catalog.check()?;
        let flag = self.observed.borrow().catalog.flag.clone();
        Ok(flag.map(|flag| Signal {
            flag: Flag(flag),
            lease: Lease::new(&self.observed),
        }))
    }
    /// Records one context abort.
    fn catalog_abort(&self) -> ExtensionResult<()> {
        self.observed.borrow().catalog.check()?;
        self.observed.borrow_mut().log("context abort");
        Ok(())
    }
    /// Whether the scripted session has queued messages.
    fn catalog_pending(&self) -> ExtensionResult<bool> {
        self.observed.borrow().catalog.check()?;
        Ok(self.observed.borrow().catalog.data.queued)
    }
    /// Records one context shutdown.
    fn catalog_shutdown(&self) -> ExtensionResult<()> {
        self.observed.borrow().catalog.check()?;
        self.observed.borrow_mut().log("context shutdown");
        Ok(())
    }
    /// The scripted context usage.
    fn catalog_usage(&self) -> ExtensionResult<Option<crate::ContextUsage>> {
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
    /// The scripted system prompt.
    fn catalog_prompt(&self) -> ExtensionResult<String> {
        self.observed.borrow().catalog.check()?;
        Ok(self.observed.borrow().catalog.data.prompt.clone())
    }
}

/// Records one dialog through the controlled host, waits while it is held pending, records
/// whether its signal is aborted by then and answers its supplied decision.
async fn ui_dialog(
    observed: &Rc<RefCell<Observed>>,
    key: usize,
    method: &str,
    operands: &serde_json::Value,
    signal: Option<&Signal>,
) -> ExtensionResult<serde_json::Value> {
    super::ui_host::call(&mut observed.borrow_mut(), key, method, operands)?;
    let hold = observed.borrow_mut().take_hold();
    pause(hold).await;
    let mut observed = observed.borrow_mut();
    if let Some(signal) = signal {
        let aborted = signal.flag.0.load(std::sync::atomic::Ordering::SeqCst);
        super::ui_host::settle(&mut observed, key, &json!(aborted));
    }
    Ok(super::ui_host::decision(&mut observed))
}

/// Dialog operands followed by whether the lent signal is aborted, or null, and the timeout.
fn dialog_operands(leading: serde_json::Value, dialog: &Dialog<'_, Signal>) -> serde_json::Value {
    let mut operands = leading;
    if let Some(operands) = operands.as_array_mut() {
        let aborted = dialog
            .signal
            .map(|signal| signal.flag.0.load(std::sync::atomic::Ordering::SeqCst));
        operands.extend([json!(aborted), json!(dialog.timeout)]);
    }
    operands
}

impl Controlled {
    /// Probe the consumed slot while its owning command still retains the identity.
    async fn repeat_continuation(&self, id: u32) {
        if self.observed.borrow().replacement.repeat != Some(true) {
            return;
        }
        let error = Exports::new(self.clone())
            .invoke_with_session(id, self.replaced_context("repeat-target"))
            .await
            .unwrap_err();
        self.observed
            .borrow_mut()
            .log(format!("replacement-repeat {error}"));
    }
}

impl crate::component_adapter::NativeImports for Controlled {
    type Watcher = super::native_watch_host::Watcher;
    fn watcher_close(&self, watch: &Self::Watcher) -> ExtensionResult<()> {
        watch.close();
        Ok(())
    }
    type Signal = Signal;
    fn fetch<'a>(
        &'a self,
        request: crate::bindings::maestro::extension::host::HttpRequest,
        signal: Option<&'a Self::Signal>,
    ) -> ExtensionFuture<'a, Self::Response> {
        Box::pin(async move {
            if signal.is_some_and(|signal| self.aborted(signal)) {
                return Err("Aborted".into());
            }
            let hold = self.observe().take_hold();
            pause(hold).await;
            if signal.is_some_and(|signal| self.aborted(signal)) {
                return Err("Aborted".into());
            }
            let mut response =
                self.observe()
                    .native_http
                    .fetch(super::native_http_host::Request {
                        url: request.url,
                        method: request.method,
                        headers: request.headers,
                        body: request.body,
                        signal: signal.map(|signal| signal.flag.key()),
                    })?;
            response.body = response
                .body
                .map(|body| body.with_signal(signal.map(|s| s.flag.shared())));
            Ok(response)
        })
    }
    fn sleep<'a>(
        &'a self,
        duration: (u64, u32),
        signal: Option<&'a Self::Signal>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            if signal.is_some_and(|signal| self.aborted(signal)) {
                return Err("Aborted".into());
            }
            self.observe()
                .log(format!("native-wait {} {}", duration.0, duration.1));
            let hold = self.observe().take_hold();
            pause(hold).await;
            if signal.is_some_and(|signal| self.aborted(signal)) {
                Err("Aborted".into())
            } else {
                Ok(())
            }
        })
    }

    type Response = super::native_http_host::Response;
    fn response_json<'a>(
        &'a self,
        response: &'a Self::Response,
    ) -> ExtensionFuture<'a, serde_json::Value> {
        Box::pin(async move { response.json() })
    }
    fn response_text<'a>(&'a self, response: &'a Self::Response) -> ExtensionFuture<'a, String> {
        Box::pin(async move { response.text() })
    }
    fn response_status_text(&self, response: &Self::Response) -> String {
        response.status_text.clone()
    }
    fn response_headers(&self, response: &Self::Response) -> Vec<(String, String)> {
        response.headers.clone()
    }
    fn response_body(&self, response: &Self::Response) -> Option<Self::Readable> {
        response.body.clone()
    }
    fn response_status(&self, response: &Self::Response) -> u16 {
        response.status
    }

    type Child = super::native_io_host::Child;
    type Readable = super::native_io_host::Reader;
    type Writable = super::native_io_host::Writer;
    fn spawn(
        &self,
        command: String,
        args: Vec<String>,
        options: crate::child_process::SpawnOptions,
    ) -> ExtensionFuture<'_, Self::Child> {
        Box::pin(async move { self.observe().native_io.spawn(command, args, options) })
    }
    fn kill(&self, pid: i32, signal: Option<String>) -> ExtensionResult<()> {
        self.observe().native_io.kill(pid, signal)
    }
    fn child_pid(&self, child: &Self::Child) -> Option<u32> {
        Some(child.pid)
    }
    fn child_stdin(&self, child: &Self::Child) -> Option<Self::Writable> {
        child.stdin.clone()
    }
    fn child_stdout(&self, child: &Self::Child) -> Option<Self::Readable> {
        child.stdout.clone()
    }
    fn child_stderr(&self, child: &Self::Child) -> Option<Self::Readable> {
        child.stderr.clone()
    }
    fn child_wait<'a>(
        &'a self,
        child: &'a Self::Child,
    ) -> ExtensionFuture<'a, crate::child_process::ExitStatus> {
        Box::pin(async move {
            let hold = self.observe().take_hold();
            pause(hold).await;
            Ok(child.wait())
        })
    }
    fn child_kill(&self, child: &Self::Child, signal: Option<String>) -> ExtensionResult<bool> {
        Ok(child.kill(signal))
    }
    fn readable_read<'a>(
        &'a self,
        reader: &'a Self::Readable,
    ) -> ExtensionFuture<'a, Option<Vec<u8>>> {
        Box::pin(async move {
            let hold = self.observe().take_hold();
            pause(hold).await;
            reader.read()
        })
    }
    fn readable_close(&self, reader: &Self::Readable) -> ExtensionResult<()> {
        reader.close();
        Ok(())
    }
    fn writable_write<'a>(
        &'a self,
        writer: &'a Self::Writable,
        bytes: &'a [u8],
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            let hold = self.observe().take_hold();
            pause(hold).await;
            writer.write(bytes)
        })
    }
    fn writable_end<'a>(&'a self, writer: &'a Self::Writable) -> ExtensionFuture<'a, ()> {
        writer
            .ended
            .store(true, std::sync::atomic::Ordering::SeqCst);
        Box::pin(std::future::ready(Ok(())))
    }
}

impl Imports for Controlled {
    fn create_coding_tools(
        &self,
        _: String,
        _: crate::ToolsOptions,
    ) -> ExtensionResult<Vec<crate::Tool>> {
        Err("built-in tools require the production host".into())
    }
    fn create_coding_tool_definitions(
        &self,
        _: String,
        _: crate::ToolsOptions,
    ) -> ExtensionResult<Vec<crate::ToolDef>> {
        Err("built-in tools require the production host".into())
    }
    fn create_read_only_tools(
        &self,
        _: String,
        _: crate::ToolsOptions,
    ) -> ExtensionResult<Vec<crate::Tool>> {
        Err("built-in tools require the production host".into())
    }
    fn create_read_only_tool_definitions(
        &self,
        _: String,
        _: crate::ToolsOptions,
    ) -> ExtensionResult<Vec<crate::ToolDef>> {
        Err("built-in tools require the production host".into())
    }
    fn create_all_tools(
        &self,
        _: String,
        _: crate::ToolsOptions,
    ) -> ExtensionResult<indexmap::IndexMap<crate::ToolName, crate::Tool>> {
        Err("built-in tools require the production host".into())
    }
    fn create_all_tool_definitions(
        &self,
        _: String,
        _: crate::ToolsOptions,
    ) -> ExtensionResult<indexmap::IndexMap<crate::ToolName, crate::ToolDef>> {
        Err("built-in tools require the production host".into())
    }

    fn create_tool(
        &self,
        _: &str,
        _: String,
        _: crate::ToolsOptions,
    ) -> ExtensionResult<crate::Tool> {
        Err("built-in tools require the production host".into())
    }
    fn create_tool_definition(
        &self,
        _: &str,
        _: String,
        _: crate::ToolsOptions,
    ) -> ExtensionResult<crate::ToolDef> {
        Err("built-in tools require the production host".into())
    }
    fn create_grep_tool(
        &self,
        _: String,
        _: crate::GrepToolOptions,
    ) -> ExtensionResult<crate::Tool> {
        Err("built-in tools require the production host".into())
    }
    fn create_grep_tool_definition(
        &self,
        _: String,
        _: crate::GrepToolOptions,
    ) -> ExtensionResult<crate::ToolDefinition> {
        Err("built-in tools require the production host".into())
    }
    fn with_file_mutation_queue(
        &self,
        _: String,
        _: Box<dyn FnOnce() -> ExtensionFuture<'static, ()>>,
    ) -> ExtensionFuture<'static, ()> {
        Box::pin(std::future::ready(Err(
            "file mutation queues require the production host".into(),
        )))
    }

    fn create_ls_tool(&self, _: String, _: crate::LsToolOptions) -> ExtensionResult<crate::Tool> {
        Err("built-in tools require the production host".into())
    }
    fn create_ls_tool_definition(
        &self,
        _: String,
        _: crate::LsToolOptions,
    ) -> ExtensionResult<crate::ToolDefinition> {
        Err("built-in tools require the production host".into())
    }
    fn create_find_tool(
        &self,
        _: String,
        _: crate::FindToolOptions,
    ) -> ExtensionResult<crate::Tool> {
        Err("built-in tools require the production host".into())
    }
    fn create_find_tool_definition(
        &self,
        _: String,
        _: crate::FindToolOptions,
    ) -> ExtensionResult<crate::ToolDefinition> {
        Err("built-in tools require the production host".into())
    }
    fn create_edit_tool(
        &self,
        _: String,
        _: crate::EditToolOptions,
    ) -> ExtensionResult<crate::Tool> {
        Err("built-in tools require the production host".into())
    }
    fn create_edit_tool_definition(
        &self,
        _: String,
        _: crate::EditToolOptions,
    ) -> ExtensionResult<crate::ToolDefinition> {
        Err("built-in tools require the production host".into())
    }
    fn create_write_tool(
        &self,
        _: String,
        _: crate::WriteToolOptions,
    ) -> ExtensionResult<crate::Tool> {
        Err("built-in tools require the production host".into())
    }
    fn create_write_tool_definition(
        &self,
        _: String,
        _: crate::WriteToolOptions,
    ) -> ExtensionResult<crate::ToolDefinition> {
        Err("built-in tools require the production host".into())
    }
    fn create_local_bash_operations(
        &self,
        _: Option<String>,
    ) -> ExtensionResult<crate::BashOperations> {
        let observed = Rc::clone(&self.observed);
        Ok(crate::BashOperations {
            exec: Rc::new(move |command, cwd, _| {
                observed.borrow_mut().native_io.launches.push((
                    command,
                    Vec::new(),
                    crate::child_process::SpawnOptions {
                        cwd: Some(cwd),
                        ..Default::default()
                    },
                ));
                Box::pin(async {
                    Ok(crate::BashExecResult {
                        exit_code: Some(0.0),
                    })
                })
            }),
        })
    }
    fn create_bash_tool(
        &self,
        _: String,
        _: crate::BashToolOptions,
    ) -> ExtensionResult<crate::Tool> {
        Err("built-in tools require the production host".into())
    }
    fn create_bash_tool_definition(
        &self,
        _: String,
        _: crate::BashToolOptions,
    ) -> ExtensionResult<crate::ToolDefinition> {
        Err("built-in tools require the production host".into())
    }
    fn create_read_tool(
        &self,
        _: String,
        _: crate::ReadToolOptions,
    ) -> ExtensionResult<crate::Tool> {
        Err("built-in tools require the production host".into())
    }
    fn create_read_tool_definition(
        &self,
        _: String,
        _: crate::ReadToolOptions,
    ) -> ExtensionResult<crate::ToolDefinition> {
        Err("built-in tools require the production host".into())
    }

    type OAuthProvider = Reader;
    fn credentials_oauth_providers(&self, owner: &Reader) -> ExtensionResult<Vec<Reader>> {
        owner.data.oauth_providers().map(|providers| {
            providers
                .into_iter()
                .map(|data| Reader {
                    data,
                    _lease: Lease::new(&self.observed),
                })
                .collect()
        })
    }
    fn oauth_info(&self, provider: &Reader) -> ExtensionResult<String> {
        provider.data.string("oauth_info")
    }
    fn oauth_login<'a>(
        &'a self,
        provider: &'a Reader,
        handler: &'a Self::Callback,
        _controls: ((bool, bool, bool), Option<&'a Self::Signal>),
    ) -> ExtensionFuture<'a, String> {
        Box::pin(async move {
            super::super::component_adapter::oauth::invoke(
                handler.id,
                crate::bindings::maestro::extension::host::OauthCallback::Progress,
                "controlled login",
                None,
            )
            .await?;
            provider
                .data
                .optional("oauth_login", &[])?
                .ok_or_else(|| "missing OAuth login result".into())
        })
    }
    fn credentials_login<'a>(
        &'a self,
        owner: &'a Reader,
        provider: String,
        _handler: &'a Self::Callback,
        _controls: ((bool, bool, bool), Option<&'a Self::Signal>),
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move { owner.data.unit("login", &[json!(provider)]) })
    }
    fn oauth_refresh<'a>(
        &'a self,
        provider: &'a Reader,
        credentials: String,
    ) -> ExtensionFuture<'a, String> {
        Box::pin(async move {
            provider
                .data
                .optional("oauth_refresh", &[serde_json::json!(credentials)])?
                .ok_or_else(|| "missing OAuth refresh result".into())
        })
    }
    fn oauth_models(&self, provider: &Reader, data: &str) -> ExtensionResult<String> {
        provider
            .data
            .optional("oauth_models", &[serde_json::json!(data)])?
            .ok_or_else(|| "missing OAuth model result".into())
    }
    fn oauth_key(&self, provider: &Reader, credentials: &str) -> ExtensionResult<String> {
        provider
            .data
            .optional("oauth_key", &[serde_json::json!(credentials)])?
            .ok_or_else(|| "missing OAuth key".into())
    }

    fn watch(&self, path: String, listener: &Self::Callback) -> ExtensionResult<Self::Watcher> {
        self.observe()
            .native_watch
            .watch(path, self.callback_id(listener))
    }
    fn native_format_size(&self, _bytes: f64) -> ExtensionResult<String> {
        serde_json::from_str(&self.observe().actions.record_for("format_size")?)
            .map_err(|error| error.to_string())
    }
    fn native_frontmatter(
        &self,
        content: &str,
    ) -> ExtensionResult<crate::bindings::maestro::extension::host::FrontmatterDocument> {
        use crate::bindings::maestro::extension::host::{FrontmatterDocument, FrontmatterToken};
        if content != "---\nx: .nan\n---\nbody" {
            return Err("unknown controlled metadata fixture".into());
        }
        Ok(FrontmatterDocument {
            values: vec![
                FrontmatterToken::Mapping(vec!["x".into()]),
                FrontmatterToken::Number(f64::NAN),
            ],
            body: "body".into(),
        })
    }
    fn native_yaml(
        &self,
        _content: &str,
    ) -> ExtensionResult<Vec<crate::bindings::maestro::extension::host::FrontmatterToken>> {
        Err("raw YAML requires the production helper host".into())
    }
    fn native_helper(&self, operation: &str, data: &str) -> ExtensionResult<String> {
        self.observe().actions.helper(operation, data)
    }
    type LockUpdate = Reader;
    fn lock_update(
        &self,
        update: &Reader,
        current: Option<&str>,
    ) -> ExtensionResult<Option<String>> {
        update
            .data
            .optional("lock_update", &[serde_json::json!(current)])
    }
    fn lock_update_async<'a>(
        &'a self,
        update: &'a Reader,
        current: Option<String>,
    ) -> ExtensionFuture<'a, Option<String>> {
        Box::pin(async move {
            update
                .data
                .optional("lock_update_async", &[serde_json::json!(current)])
        })
    }
    fn auth_storage_from_backend(&self, backend: &Self::Callback) -> ExtensionResult<Reader> {
        let data = self.observed.borrow().readers.acquire()?.fresh();
        data.unit("auth_storage_from_backend", &[])?;
        let resource = Reader {
            data,
            _lease: Lease::new(&self.observed),
        };
        let id = self.callback_id(backend);
        resource.data.1.lock().unwrap().backend = Some(id);
        self.observe().register("credential-backend".into(), id);
        self.credential_backend(&resource)?;
        Ok(resource)
    }
    fn credentials_set_fallback(
        &self,
        owner: &Reader,
        resolver: &Self::Callback,
    ) -> ExtensionResult<()> {
        owner.data.unit("set_fallback_resolver", &[])?;
        let id = self.callback_id(resolver);
        owner.data.1.lock().unwrap().fallback = Some(id);
        self.observe().register("credential-fallback".into(), id);
        Ok(())
    }
    type Credentials = Reader;
    fn create_auth_storage(&self, path: Option<&str>) -> ExtensionResult<Reader> {
        let data = self.observed.borrow().readers.acquire()?.fresh();
        data.unit("create_auth_storage", &[serde_json::json!(path)])?;
        Ok(Reader {
            data,
            _lease: Lease::new(&self.observed),
        })
    }
    fn auth_storage_in_memory(&self, data: &str) -> ExtensionResult<Reader> {
        let reader = self.observed.borrow().readers.acquire()?.fresh();
        reader.unit("auth_storage_in_memory", &[serde_json::json!(data)])?;
        Ok(Reader {
            data: reader,
            _lease: Lease::new(&self.observed),
        })
    }
    fn create_model_registry(&self, auth: &Reader, path: Option<&str>) -> ExtensionResult<Reader> {
        auth.data
            .unit("create_model_registry", &[serde_json::json!(path)])?;
        Ok(Reader {
            data: auth.data.clone(),
            _lease: Lease::new(&self.observed),
        })
    }
    fn in_memory_model_registry(&self, auth: &Reader) -> ExtensionResult<Reader> {
        auth.data.unit("in_memory_model_registry", &[])?;
        Ok(Reader {
            data: auth.data.clone(),
            _lease: Lease::new(&self.observed),
        })
    }
    fn registry_auth_storage(&self, resource: &Reader) -> ExtensionResult<Reader> {
        Ok(Reader {
            data: resource.data.clone(),
            _lease: Lease::new(&self.observed),
        })
    }
    fn credentials_set_runtime_api_key(
        &self,
        resource: &Reader,
        provider: &str,
        key: &str,
    ) -> ExtensionResult<()> {
        resource.data.unit(
            "set_runtime_api_key",
            &[serde_json::json!(provider), serde_json::json!(key)],
        )
    }
    fn credentials_remove_runtime_api_key(
        &self,
        resource: &Reader,
        provider: &str,
    ) -> ExtensionResult<()> {
        resource
            .data
            .unit("remove_runtime_api_key", &[serde_json::json!(provider)])
    }
    fn credentials_reload(&self, resource: &Reader) -> ExtensionResult<()> {
        resource.data.unit("reload", &[])?;
        self.credential_backend(resource)
    }
    fn credentials_get(
        &self,
        resource: &Reader,
        provider: &str,
    ) -> ExtensionResult<Option<String>> {
        resource.data.record("get", &[serde_json::json!(provider)])
    }
    fn credentials_set(
        &self,
        resource: &Reader,
        provider: &str,
        credential: &str,
    ) -> ExtensionResult<()> {
        resource.data.unit(
            "set",
            &[serde_json::json!(provider), serde_json::json!(credential)],
        )?;
        self.credential_backend(resource)
    }
    fn credentials_remove(&self, resource: &Reader, provider: &str) -> ExtensionResult<()> {
        resource.data.unit("remove", &[serde_json::json!(provider)])
    }
    fn credentials_list(&self, resource: &Reader) -> ExtensionResult<Vec<String>> {
        serde_json::from_str(
            &resource
                .data
                .record("list", &[])?
                .ok_or("missing credential list")?,
        )
        .map_err(|error| error.to_string())
    }
    fn credentials_has(&self, resource: &Reader, provider: &str) -> ExtensionResult<bool> {
        resource.data.flag("has", &[serde_json::json!(provider)])
    }
    fn credentials_has_auth(&self, resource: &Reader, provider: &str) -> ExtensionResult<bool> {
        Self::credential_fallback(resource, provider)?;
        resource
            .data
            .flag("has_auth", &[serde_json::json!(provider)])
    }
    fn credentials_get_auth_status(
        &self,
        resource: &Reader,
        provider: &str,
    ) -> ExtensionResult<String> {
        Self::credential_fallback(resource, provider)?;
        resource
            .data
            .record("get_auth_status", &[serde_json::json!(provider)])?
            .ok_or("missing credential record".into())
    }
    fn credentials_get_all(&self, resource: &Reader) -> ExtensionResult<String> {
        resource
            .data
            .record("get_all", &[])?
            .ok_or("missing credential record".into())
    }
    fn credentials_drain_errors(&self, resource: &Reader) -> ExtensionResult<String> {
        resource
            .data
            .record("drain_errors", &[])?
            .ok_or("missing credential record".into())
    }
    fn credentials_logout(&self, resource: &Reader, provider: &str) -> ExtensionResult<()> {
        resource.data.unit("logout", &[serde_json::json!(provider)])
    }
    fn credentials_get_api_key<'a>(
        &'a self,
        resource: &'a Reader,
        provider: String,
        include_fallback: Option<bool>,
    ) -> ExtensionFuture<'a, Option<String>> {
        Box::pin(async move {
            let hold = self.observe().take_hold();
            pause(hold).await;
            if include_fallback != Some(false) {
                Self::credential_fallback(resource, &provider)?;
            }
            resource.data.optional(
                "get_api_key",
                &[
                    serde_json::json!(provider),
                    serde_json::json!(include_fallback),
                ],
            )
        })
    }

    type BashOutput = super::shell_host::NativeReceiver;
    fn bash_output(&self, output: &Self::BashOutput, bytes: &[u8]) -> ExtensionResult<()> {
        self.observe().shell.outputs[output.id].data(bytes)
    }

    fn fork<'a>(
        &'a self,
        context: &'a CommandContext,
        id: &'a str,
        data: Option<String>,
        callback: Option<&'a Callback>,
    ) -> ExtensionFuture<'a, SessionChangeResult> {
        Box::pin(async move {
            let _completion = super::replacement_host::Completion(
                self.observed.borrow_mut().replacement_finished.take(),
            );
            self.observed.borrow_mut().log(format!(
                "replacement-binding {}",
                serde_json::json!(context.session.cwd)
            ));
            let hold = super::replacement_host::start(
                &mut self.observed.borrow_mut(),
                "fork",
                id,
                data.as_deref(),
                callback.is_some(),
            );
            pause(hold).await;
            let cancelled = super::replacement_host::outcome(&self.observed.borrow())?;
            self.observed.borrow_mut().log("replacement-rebound");
            if !cancelled
                && !self.observed.borrow().replacement.unused
                && let Some(callback) = callback
            {
                let mut replaced = self.replaced_context(id);
                replaced.setup = Some(super::setup_host::reader(id));
                Exports::new(self.clone())
                    .invoke_with_session(callback.id, replaced)
                    .await?;
                self.repeat_continuation(callback.id).await;
            }
            let _ = context;
            Ok(SessionChangeResult { cancelled })
        })
    }
    fn navigate_tree<'a>(
        &'a self,
        context: &'a CommandContext,
        id: &'a str,
        data: Option<String>,
    ) -> ExtensionFuture<'a, SessionChangeResult> {
        Box::pin(async move {
            let _completion = super::replacement_host::Completion(
                self.observed.borrow_mut().replacement_finished.take(),
            );
            self.observed.borrow_mut().log(format!(
                "replacement-binding {}",
                serde_json::json!(context.session.cwd)
            ));
            let hold = super::replacement_host::start(
                &mut self.observed.borrow_mut(),
                "navigateTree",
                id,
                data.as_deref(),
                false,
            );
            pause(hold).await;
            super::replacement_host::outcome(&self.observed.borrow())
                .map(|cancelled| SessionChangeResult { cancelled })
        })
    }
    fn switch_session<'a>(
        &'a self,
        context: &'a CommandContext,
        id: &'a str,
        options: Option<()>,
        callback: Option<&'a Callback>,
    ) -> ExtensionFuture<'a, SessionChangeResult> {
        Box::pin(async move {
            let _completion = super::replacement_host::Completion(
                self.observed.borrow_mut().replacement_finished.take(),
            );
            self.observed.borrow_mut().log(format!(
                "replacement-binding {}",
                serde_json::json!(context.session.cwd)
            ));
            let hold = super::replacement_host::start(
                &mut self.observed.borrow_mut(),
                "switchSession",
                id,
                options.map(|()| "{}"),
                callback.is_some(),
            );
            pause(hold).await;
            let cancelled = super::replacement_host::outcome(&self.observed.borrow())?;
            self.observed.borrow_mut().log("replacement-rebound");
            if !cancelled
                && !self.observed.borrow().replacement.unused
                && let Some(callback) = callback
            {
                let mut replaced = self.replaced_context(id);
                replaced.setup = Some(super::setup_host::reader(id));
                Exports::new(self.clone())
                    .invoke_with_session(callback.id, replaced)
                    .await?;
                self.repeat_continuation(callback.id).await;
            }
            Ok(SessionChangeResult { cancelled })
        })
    }
    fn reload<'a>(&'a self, context: &'a CommandContext) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            let _completion = super::replacement_host::Completion(
                self.observed.borrow_mut().replacement_finished.take(),
            );
            self.observed.borrow_mut().log(format!(
                "replacement-binding {}",
                serde_json::json!(context.session.cwd)
            ));
            let hold = super::replacement_host::start(
                &mut self.observed.borrow_mut(),
                "reload",
                "",
                None,
                false,
            );
            pause(hold).await;
            super::replacement_host::outcome(&self.observed.borrow()).map(|_| ())
        })
    }
    fn replaced_send_message<'a>(
        &'a self,
        context: &'a ReplacedContext,
        payload: String,
        options: Option<String>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            let hold = super::replacement_host::send_start(
                &mut self.observed.borrow_mut(),
                "sendMessage",
                &context.session.cwd,
            );
            pause(hold).await;
            super::replacement_host::send(
                &mut self.observed.borrow_mut(),
                context.setup.as_ref().unwrap(),
                ("sendMessage", &context.session.cwd),
                &payload,
                options.as_deref(),
            )
        })
    }
    fn replaced_send_user_message<'a>(
        &'a self,
        context: &'a ReplacedContext,
        payload: String,
        options: Option<String>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            let hold = super::replacement_host::send_start(
                &mut self.observed.borrow_mut(),
                "sendUserMessage",
                &context.session.cwd,
            );
            pause(hold).await;
            super::replacement_host::send(
                &mut self.observed.borrow_mut(),
                context.setup.as_ref().unwrap(),
                ("sendUserMessage", &context.session.cwd),
                &payload,
                options.as_deref(),
            )
        })
    }

    fn compact(
        &self,
        context: &Context,
        instructions: Option<&CompactionData>,
        complete: Option<&Callback>,
        error: Option<&Callback>,
    ) -> ExtensionResult<()> {
        self.compact_binding(&context.session, instructions, complete, error)
    }
    fn command_compact(
        &self,
        context: &CommandContext,
        instructions: Option<&CompactionData>,
        complete: Option<&Callback>,
        error: Option<&Callback>,
    ) -> ExtensionResult<()> {
        self.compact_binding(&context.session, instructions, complete, error)
    }

    fn send_message(&self, message: &str, options: Option<&str>) -> ExtensionResult<()> {
        super::action_host::action(
            &mut self.observed.borrow_mut(),
            "sendMessage",
            &serde_json::json!([
                super::action_host::document(Some(message)),
                super::action_host::document(options)
            ]),
        )
    }
    fn send_user_message(&self, content: &str, options: Option<&str>) -> ExtensionResult<()> {
        super::action_host::action(
            &mut self.observed.borrow_mut(),
            "sendUserMessage",
            &serde_json::json!([
                super::action_host::document(Some(content)),
                super::action_host::document(options)
            ]),
        )
    }
    fn set_session_name(&self, name: &str) -> ExtensionResult<()> {
        super::action_host::action(
            &mut self.observed.borrow_mut(),
            "setSessionName",
            &serde_json::json!([name]),
        )
    }
    fn set_label(&self, entry_id: &str, label: Option<&str>) -> ExtensionResult<()> {
        super::action_host::action(
            &mut self.observed.borrow_mut(),
            "setLabel",
            &serde_json::json!([
                entry_id,
                label.map_or(
                    serde_json::json!({"present":false}),
                    |v| serde_json::json!({"present":true,"value":v})
                )
            ]),
        )
    }
    fn set_active_tools(&self, tool_names: &[String]) -> ExtensionResult<()> {
        super::action_host::action(
            &mut self.observed.borrow_mut(),
            "setActiveTools",
            &serde_json::json!([tool_names]),
        )
    }
    fn set_thinking_level(&self, level: &str) -> ExtensionResult<()> {
        super::action_host::action(
            &mut self.observed.borrow_mut(),
            "setThinkingLevel",
            &serde_json::json!([serde_json::from_str::<serde_json::Value>(level).unwrap()]),
        )
    }
    fn get_session_name(&self) -> ExtensionResult<Option<String>> {
        let o = self.observed.borrow();
        o.actions.check()?;
        Ok(o.actions.value.as_str().map(str::to_owned))
    }
    fn get_active_tools(&self) -> ExtensionResult<Vec<String>> {
        let o = self.observed.borrow();
        o.actions.check()?;
        serde_json::from_value(
            o.actions
                .value
                .get("getActiveTools")
                .unwrap_or(&o.actions.value)
                .clone(),
        )
        .map_err(|e| e.to_string())
    }
    fn schema_invoke(
        &self,
        _: &Self::Callback,
        _: crate::bindings::maestro::extension::host::SchemaOperation,
        _: &Self::JsonNode,
        _: &str,
    ) -> crate::schema_code::SchemaResult<String> {
        Err(crate::schema_code::DiagnosticErrorInfo::error(
            "foreign schemas use the production runtime",
        ))
    }
    fn get_all_tools(
        &self,
    ) -> ExtensionResult<crate::component_adapter::ToolListing<Self::Callback>> {
        let (metadata, bindings) = super::schema_host::listing(&self.observed.borrow())?;
        let mut retained = Vec::new();
        for (tool, pointer, id) in bindings {
            let (new_id, handle) = self.new_callback();
            Exports::new(self.clone())
                .retain_schema(
                    id,
                    Callback {
                        id: new_id,
                        observed: Rc::clone(&self.observed),
                    },
                )
                .map_err(|error| error.message)?;
            let shape = Exports::<Self>::describe_schema(id).map_err(|error| error.message)?;
            retained.push((tool, pointer, handle, shape, true));
        }
        Ok((metadata, retained))
    }
    fn get_commands(&self) -> ExtensionResult<String> {
        self.observed.borrow().actions.record()
    }
    fn get_thinking_level(&self) -> ExtensionResult<String> {
        self.observed
            .borrow()
            .actions
            .record_for("getThinkingLevel")
    }
    fn call<'a>(
        &'a self,
        operation: &'a str,
        data: String,
        _signal: Option<&'a Self::Signal>,
    ) -> ExtensionFuture<'a, String> {
        Box::pin(async move {
            let data: serde_json::Value =
                serde_json::from_str(&data).map_err(|error| error.to_string())?;
            super::action_host::action(&mut self.observed.borrow_mut(), operation, &data)?;
            let hold = self.observed.borrow_mut().take_hold();
            pause(hold).await;
            self.observed.borrow().actions.record()
        })
    }

    fn set_model(&self, model: String) -> ExtensionFuture<'_, bool> {
        Box::pin(async move {
            self.observed.borrow_mut().log("model selection started");
            let hold = self.observed.borrow_mut().take_hold();
            pause(hold).await;
            super::action_host::action(
                &mut self.observed.borrow_mut(),
                "setModel",
                &serde_json::json!([serde_json::from_str::<serde_json::Value>(&model).unwrap()]),
            )?;
            self.observed.borrow().actions.boolean("setModel")
        })
    }

    type Subscription = super::bus_host::Subscription;
    type Emitter = super::bus_host::Emitter;
    type JsonNode = super::schema_host::JsonNode;
    fn json_kind(
        &self,
        node: &Self::JsonNode,
        path: &str,
    ) -> ExtensionResult<Option<crate::schema_code::JsonKind>> {
        node.kind(path)
    }
    fn json_read(&self, node: &Self::JsonNode, path: &str) -> ExtensionResult<Option<String>> {
        node.read(path)
    }
    fn json_keys(&self, node: &Self::JsonNode, path: &str) -> ExtensionResult<Vec<String>> {
        node.keys(path)
    }
    fn json_set(
        &self,
        node: &Self::JsonNode,
        path: &str,
        key: &str,
        value: &str,
    ) -> ExtensionResult<()> {
        node.set(path, key, value)
    }
    fn json_delete(&self, node: &Self::JsonNode, path: &str, key: &str) -> ExtensionResult<()> {
        node.delete(path, key)
    }
    fn json_splice(
        &self,
        node: &Self::JsonNode,
        path: &str,
        range: (u32, u32),
        values: &[String],
    ) -> ExtensionResult<Vec<String>> {
        node.splice(path, range, values)
    }
    fn callback_id(&self, callback: &Self::Callback) -> u32 {
        callback.id
    }

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
        Box::pin(async move {
            let hold = self.observed.borrow_mut().bus.emission_holds.pop_front();
            super::observed::pause(hold).await;
            self.callback_emit(&super::bus_host::Emitter { key: 0 }, &channel, &data)
        })
    }
    fn callback_emit(
        &self,
        emitter: &Self::Emitter,
        channel: &str,
        data: &str,
    ) -> ExtensionResult<()> {
        if emitter.key != 0 {
            self.observed.borrow_mut().bus.renderer_emissions.push((
                emitter.key,
                channel.into(),
                serde_json::from_str(data).unwrap(),
            ));
        }
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

    fn writer_reader(&self, resource: &Reader) -> Reader {
        Reader {
            data: resource.data.clone(),
            _lease: Lease::new(&self.observed),
        }
    }
    fn writer_set_session_file(&self, resource: &Reader, path: &str) -> ExtensionResult<()> {
        let _value = super::setup_host::call(
            &mut self.observed.borrow_mut(),
            &resource.data,
            "set_session_file",
            &serde_json::json!([path]),
        )?;
        Ok(())
    }
    fn writer_new_session(
        &self,
        resource: &Reader,
        options: Option<&str>,
    ) -> ExtensionResult<Option<String>> {
        let value = super::setup_host::call(
            &mut self.observed.borrow_mut(),
            &resource.data,
            "new_session",
            &serde_json::json!([document(options)]),
        )?;
        Ok(value.as_str().map(str::to_owned))
    }
    fn writer_is_persisted(&self, resource: &Reader) -> ExtensionResult<bool> {
        let value = super::setup_host::call(
            &mut self.observed.borrow_mut(),
            &resource.data,
            "is_persisted",
            &serde_json::json!([]),
        )?;
        Ok(value.as_bool().unwrap())
    }
    fn writer_get_children(&self, resource: &Reader, parent_id: &str) -> ExtensionResult<String> {
        let value = super::setup_host::call(
            &mut self.observed.borrow_mut(),
            &resource.data,
            "get_children",
            &serde_json::json!([parent_id]),
        )?;
        let raw = self.observed.borrow().setup.raw.clone();
        Ok(raw.unwrap_or_else(|| value.to_string()))
    }
    fn writer_build_session_context(&self, resource: &Reader) -> ExtensionResult<String> {
        let value = super::setup_host::call(
            &mut self.observed.borrow_mut(),
            &resource.data,
            "build_session_context",
            &serde_json::json!([]),
        )?;
        let raw = self.observed.borrow().setup.raw.clone();
        Ok(raw.unwrap_or_else(|| value.to_string()))
    }
    fn writer_append_message(&self, resource: &Reader, message: &str) -> ExtensionResult<String> {
        let value = super::setup_host::call(
            &mut self.observed.borrow_mut(),
            &resource.data,
            "append_message",
            &serde_json::json!([document(Some(message))]),
        )?;
        Ok(value.as_str().unwrap().to_owned())
    }
    fn writer_append_thinking_level_change(
        &self,
        resource: &Reader,
        level: &str,
    ) -> ExtensionResult<String> {
        let value = super::setup_host::call(
            &mut self.observed.borrow_mut(),
            &resource.data,
            "append_thinking_level_change",
            &serde_json::json!([level]),
        )?;
        Ok(value.as_str().unwrap().to_owned())
    }
    fn writer_append_model_change(
        &self,
        resource: &Reader,
        provider: &str,
        model_id: &str,
    ) -> ExtensionResult<String> {
        let value = super::setup_host::call(
            &mut self.observed.borrow_mut(),
            &resource.data,
            "append_model_change",
            &serde_json::json!([provider, model_id]),
        )?;
        Ok(value.as_str().unwrap().to_owned())
    }
    fn writer_append_compaction(
        &self,
        resource: &Reader,
        compaction: &str,
        from_hook: Option<bool>,
    ) -> ExtensionResult<String> {
        let value = super::setup_host::call(
            &mut self.observed.borrow_mut(),
            &resource.data,
            "append_compaction",
            &serde_json::json!([document(Some(compaction)), from_hook]),
        )?;
        Ok(value.as_str().unwrap().to_owned())
    }
    fn writer_append_custom_entry(
        &self,
        resource: &Reader,
        custom_type: &str,
        data: Option<&str>,
    ) -> ExtensionResult<String> {
        let value = super::setup_host::call(
            &mut self.observed.borrow_mut(),
            &resource.data,
            "append_custom_entry",
            &serde_json::json!([custom_type, document(data)]),
        )?;
        Ok(value.as_str().unwrap().to_owned())
    }
    fn writer_append_session_info(&self, resource: &Reader, name: &str) -> ExtensionResult<String> {
        let value = super::setup_host::call(
            &mut self.observed.borrow_mut(),
            &resource.data,
            "append_session_info",
            &serde_json::json!([name]),
        )?;
        Ok(value.as_str().unwrap().to_owned())
    }
    fn writer_append_custom_message_entry(
        &self,
        resource: &Reader,
        message: &str,
    ) -> ExtensionResult<String> {
        let value = super::setup_host::call(
            &mut self.observed.borrow_mut(),
            &resource.data,
            "append_custom_message_entry",
            &serde_json::json!([document(Some(message))]),
        )?;
        Ok(value.as_str().unwrap().to_owned())
    }
    fn writer_append_label_change(
        &self,
        resource: &Reader,
        target_id: &str,
        label: Option<&str>,
    ) -> ExtensionResult<String> {
        let value = super::setup_host::call(
            &mut self.observed.borrow_mut(),
            &resource.data,
            "append_label_change",
            &serde_json::json!([target_id, label]),
        )?;
        Ok(value.as_str().unwrap().to_owned())
    }
    fn writer_branch(&self, resource: &Reader, from_id: &str) -> ExtensionResult<()> {
        let _value = super::setup_host::call(
            &mut self.observed.borrow_mut(),
            &resource.data,
            "branch",
            &serde_json::json!([from_id]),
        )?;
        Ok(())
    }
    fn writer_reset_leaf(&self, resource: &Reader) -> ExtensionResult<()> {
        let _value = super::setup_host::call(
            &mut self.observed.borrow_mut(),
            &resource.data,
            "reset_leaf",
            &serde_json::json!([]),
        )?;
        Ok(())
    }
    fn writer_branch_with_summary(
        &self,
        resource: &Reader,
        from_id: Option<&str>,
        summary: &str,
        metadata: (Option<&str>, Option<bool>),
    ) -> ExtensionResult<String> {
        let value = super::setup_host::call(
            &mut self.observed.borrow_mut(),
            &resource.data,
            "branch_with_summary",
            &serde_json::json!([from_id, summary, document(metadata.0), metadata.1]),
        )?;
        Ok(value.as_str().unwrap().to_owned())
    }
    fn writer_create_branched_session(
        &self,
        resource: &Reader,
        leaf_id: &str,
    ) -> ExtensionResult<Option<String>> {
        let value = super::setup_host::call(
            &mut self.observed.borrow_mut(),
            &resource.data,
            "create_branched_session",
            &serde_json::json!([leaf_id]),
        )?;
        Ok(value.as_str().map(str::to_owned))
    }

    type Ui = Ui;
    type CustomClose = super::custom_host::Close;
    fn custom_close(&self, close: &Self::CustomClose) -> ExtensionResult<bool> {
        Ok(close.close())
    }
    fn ui_custom<'a>(
        &'a self,
        resource: &'a Ui,
        factory: &'a Callback,
        options: Option<crate::component_adapter::custom::CustomOptionsRef<'a, Self>>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            ui_call(&self.observed, resource.key, "custom", &json!([]))?;
            match options {
                Some(options) if options.overlay == Some(true) => {
                    self.custom_overlay(factory, options).await
                }
                _ => self.custom(factory).await,
            }
        })
    }
    type OverlayOptions = OverlayOptions;
    fn new_overlay_options(&self, encoded: &str) -> ExtensionResult<OverlayOptions> {
        Ok(OverlayOptions {
            value: super::overlay_host::Options::new(encoded)?,
            visible: Rc::default(),
            _lease: Lease::new(&self.observed),
        })
    }
    fn overlay_options_get(&self, options: &OverlayOptions) -> ExtensionResult<String> {
        options.value.get()
    }
    fn overlay_options_set(&self, options: &OverlayOptions, encoded: &str) -> ExtensionResult<()> {
        options.value.set(encoded)
    }
    fn overlay_options_share(&self, options: &OverlayOptions) -> OverlayOptions {
        OverlayOptions {
            value: options.value.clone(),
            visible: options.visible.clone(),
            _lease: Lease::new(&self.observed),
        }
    }
    fn overlay_options_set_visible(
        &self,
        options: &OverlayOptions,
        visible: Option<&Callback>,
    ) -> ExtensionResult<()> {
        self.observe()
            .log(format!("overlay visible {}", visible.is_some()));
        options.visible.set(visible.map(|visible| visible.id));
        Ok(())
    }
    type OverlayHandle = Tui;
    fn overlay_handle_request(
        &self,
        _: &Tui,
        request: crate::component_adapter::overlays::OverlayRequest,
    ) -> ExtensionResult<Option<bool>> {
        use crate::component_adapter::overlays::OverlayRequest;
        let (line, flag) = match request {
            OverlayRequest::Hide => ("hide".to_owned(), None),
            OverlayRequest::SetHidden(hidden) => (format!("setHidden {hidden}"), None),
            OverlayRequest::IsHidden => ("isHidden".to_owned(), Some(false)),
            OverlayRequest::Focus => ("focus".to_owned(), None),
            OverlayRequest::Unfocus => ("unfocus".to_owned(), None),
            OverlayRequest::IsFocused => ("isFocused".to_owned(), Some(true)),
        };
        self.observe().log(format!("overlay handle {line}"));
        Ok(flag)
    }
    type Tui = Tui;
    fn tui_full_redraws(&self, _: &Tui) -> ExtensionResult<u64> {
        self.observe().log("tui fullRedraws");
        Ok(17)
    }
    type Invalidator = Invalidator;
    fn invalidate(&self, invalidator: &Invalidator) -> ExtensionResult<()> {
        let mut observed = self.observed.borrow_mut();
        observed.log(format!("invalidate {}", invalidator.row));
        Ok(())
    }
    type FooterData = FooterData;
    fn footer_get_git_branch(&self, _: &FooterData) -> ExtensionResult<Option<String>> {
        let branch = self.observed.borrow().ui.footer.read("getGitBranch");
        Ok(branch.as_str().map(str::to_owned))
    }
    fn footer_get_extension_statuses(
        &self,
        _: &FooterData,
    ) -> ExtensionResult<Vec<(String, String)>> {
        let statuses = self
            .observed
            .borrow()
            .ui
            .footer
            .read("getExtensionStatuses");
        serde_json::from_value(statuses).map_err(|error| error.to_string())
    }
    fn footer_get_available_provider_count(&self, _: &FooterData) -> ExtensionResult<f64> {
        let count = self
            .observed
            .borrow()
            .ui
            .footer
            .read("getAvailableProviderCount");
        Ok(count.as_f64().unwrap_or_default())
    }
    fn footer_on_branch_change(
        &self,
        _: &FooterData,
        listener: &Callback,
    ) -> ExtensionResult<Self::Subscription> {
        self.observed.borrow_mut().bus.on("branch", listener.id)
    }
    fn ui_notify(
        &self,
        resource: &Ui,
        message: &str,
        kind: Option<crate::NotificationType>,
    ) -> ExtensionResult<()> {
        let seen = kind.map(super::ui_host::variant);
        ui_call(
            &self.observed,
            resource.key,
            "notify",
            &super::ui_host::notify(message, seen.as_deref()),
        )
    }
    fn ui_set_status(&self, resource: &Ui, key: &str, text: Option<&str>) -> ExtensionResult<()> {
        ui_call(
            &self.observed,
            resource.key,
            "setStatus",
            &super::ui_host::status(key, text),
        )
    }
    fn ui_set_working_message(&self, resource: &Ui, message: Option<&str>) -> ExtensionResult<()> {
        ui_call(
            &self.observed,
            resource.key,
            "setWorkingMessage",
            &super::ui_host::working_message(message),
        )
    }
    fn ui_set_working_visible(&self, resource: &Ui, visible: bool) -> ExtensionResult<()> {
        ui_call(
            &self.observed,
            resource.key,
            "setWorkingVisible",
            &super::ui_host::working_visible(visible),
        )
    }
    fn ui_set_working_indicator(
        &self,
        resource: &Ui,
        options: Option<crate::WorkingIndicatorOptions>,
    ) -> ExtensionResult<()> {
        let options = options.map(|options| super::ui_host::IndicatorOptions {
            frames: options.frames,
            interval_ms: options.interval_ms,
        });
        ui_call(
            &self.observed,
            resource.key,
            "setWorkingIndicator",
            &super::ui_host::indicator(options),
        )
    }
    fn ui_set_hidden_thinking_label(
        &self,
        resource: &Ui,
        label: Option<&str>,
    ) -> ExtensionResult<()> {
        ui_call(
            &self.observed,
            resource.key,
            "setHiddenThinkingLabel",
            &super::ui_host::hidden_thinking_label(label),
        )
    }
    fn ui_set_widget(
        &self,
        resource: &Ui,
        key: &str,
        content: Option<&[String]>,
        options: Option<crate::ExtensionWidgetOptions>,
    ) -> ExtensionResult<()> {
        let options = options.map(|options| super::ui_host::WidgetOptions {
            placement: options.placement.map(super::ui_host::variant),
        });
        ui_call(
            &self.observed,
            resource.key,
            "setWidget",
            &super::ui_host::widget(key, content, options),
        )?;
        self.take_widgets(key)
    }
    fn ui_set_widget_factory(
        &self,
        resource: &Ui,
        key: &str,
        factory: &Callback,
        options: Option<crate::ExtensionWidgetOptions>,
    ) -> ExtensionResult<()> {
        let placement = options
            .and_then(|options| options.placement)
            .map_or_else(|| "AboveEditor".to_owned(), super::ui_host::variant);
        let operands = json!([key, placement]);
        ui_call(&self.observed, resource.key, "setWidgetFactory", &operands)?;
        self.take_widgets(key)?;
        let handle = self.build(factory, false)?;
        self.observed
            .borrow_mut()
            .ui
            .widgets
            .push(super::ui_host::WidgetSlot {
                key: key.to_owned(),
                component: handle,
                dispose: None,
                set_expanded: None,
                width: None,
            });
        Ok(())
    }
    fn ui_set_footer(&self, resource: &Ui, factory: Option<&Callback>) -> ExtensionResult<()> {
        let operands = json!([factory.is_some()]);
        ui_call(&self.observed, resource.key, "setFooter", &operands)?;
        let old = self.observed.borrow_mut().ui.footer.custom.take();
        let old = old.and_then(|old| self.widgets.borrow_mut().remove(&old.component));
        if let Some(dispose) = old.as_ref().and_then(|old| old.dispose.clone()) {
            self.dispose(&dispose)?;
        }
        drop(old);
        if let Some(factory) = factory {
            let component = self.build(factory, true)?;
            self.observed.borrow_mut().ui.footer.factory = Some(factory.id);
            self.observed.borrow_mut().ui.footer.custom = Some(super::ui_host::WidgetSlot {
                key: "footer".to_owned(),
                component,
                dispose: None,
                set_expanded: None,
                width: None,
            });
        }
        Ok(())
    }
    fn ui_set_header(&self, resource: &Ui, factory: Option<&Callback>) -> ExtensionResult<()> {
        let operands = json!([factory.is_some()]);
        ui_call(&self.observed, resource.key, "setHeader", &operands)?;
        if !self.observed.borrow().ui.header.ready {
            return Ok(());
        }
        let old = self
            .observed
            .borrow()
            .ui
            .header
            .custom
            .as_ref()
            .map(|slot| slot.component);
        let old = old.and_then(|old| self.widgets.borrow_mut().remove(&old));
        if let Some(dispose) = old.as_ref().and_then(|old| old.dispose.clone()) {
            self.dispose(&dispose)?;
        }
        drop(old);
        let slot = match factory {
            None => None,
            Some(factory) => Some(super::ui_host::WidgetSlot {
                key: "header".to_owned(),
                component: self.build(factory, false)?,
                dispose: None,
                set_expanded: None,
                width: None,
            }),
        };
        self.observed.borrow_mut().ui.header.install(slot);
        Ok(())
    }
    fn ui_set_title(&self, resource: &Ui, title: &str) -> ExtensionResult<()> {
        ui_call(
            &self.observed,
            resource.key,
            "setTitle",
            &super::ui_host::title(title),
        )
    }
    fn ui_paste_to_editor(&self, resource: &Ui, text: &str) -> ExtensionResult<()> {
        ui_call(
            &self.observed,
            resource.key,
            "pasteToEditor",
            &serde_json::json!([text]),
        )
    }
    fn ui_set_editor_text(&self, resource: &Ui, text: &str) -> ExtensionResult<()> {
        super::ui_host::set_editor_text(&mut self.observed.borrow_mut(), resource.key, text)
    }
    fn ui_get_editor_text(&self, resource: &Ui) -> ExtensionResult<String> {
        super::ui_host::get_editor_text(&mut self.observed.borrow_mut(), resource.key)
    }
    fn ui_set_editor_component(
        &self,
        resource: &Ui,
        factory: Option<&Callback>,
    ) -> ExtensionResult<()> {
        let target = factory.map(|factory| factory.id);
        let old = super::ui_host::set_editor_component(
            &mut self.observed.borrow_mut(),
            resource.key,
            target,
        )?;
        if let Some(old) = old {
            let mut observed = self.observed.borrow_mut();
            observed.register(format!("editor component {}", old.component), old.component);
            observed.register(format!("editor {}", old.editor), old.editor);
        }
        target.map_or(Ok(()), |factory| self.build_editor_component(factory))
    }
    fn ui_add_autocomplete_provider(
        &self,
        resource: &Ui,
        factory: &Callback,
    ) -> ExtensionResult<()> {
        let current = super::provider_host::add(&mut self.observed.borrow_mut(), resource.key)?;
        let emitter = super::bus_host::Emitter { key: 0 };
        let exports = Exports::new(self.clone());
        let provider = exports.invoke_provider_factory(factory.id, current, &emitter)?;
        let id = crate::component_adapter::keep_provider(self, provider);
        super::provider_host::push(&mut self.observed.borrow_mut(), id);
        self.observe()
            .register(format!("provider factory {id}"), factory.id);
        Ok(())
    }
    fn ui_get_editor_component(
        &self,
        resource: &Ui,
    ) -> ExtensionResult<crate::component_adapter::CurrentFactory> {
        let current =
            super::ui_host::get_editor_component(&mut self.observed.borrow_mut(), resource.key)?;
        Ok(current.map(|current| {
            current.map_err(|custom| {
                std::rc::Rc::new(super::widget_controlled::ForeignFactory(custom))
                    as std::rc::Rc<dyn std::any::Any>
            })
        }))
    }
    fn ui_theme(&self, resource: &Ui) -> ExtensionResult<Theme> {
        let key = self.ui_themed(resource, "theme", &serde_json::Value::Null)?;
        Ok(self.theme(key.as_str().unwrap_or_default()))
    }
    fn ui_get_all_themes(&self, resource: &Ui) -> ExtensionResult<Vec<crate::ThemeInfo>> {
        self.ui_themed(resource, "getAllThemes", &serde_json::Value::Null)?;
        Ok(super::ui_host::THEMES
            .iter()
            .map(|(name, path)| crate::ThemeInfo {
                name: (*name).to_owned(),
                path: path.map(str::to_owned),
            })
            .collect())
    }
    fn ui_get_theme(&self, resource: &Ui, name: &str) -> ExtensionResult<Option<Theme>> {
        let loaded = self.ui_themed(resource, "getTheme", &json!(name))?;
        Ok(loaded.as_str().map(|key| self.theme(key)))
    }
    fn ui_set_theme_name(
        &self,
        resource: &Ui,
        name: &str,
    ) -> ExtensionResult<crate::ThemeSetResult> {
        let result = self.ui_themed(resource, "setThemeName", &json!(name))?;
        Ok(theme_set_result(&result))
    }
    fn ui_set_theme(&self, resource: &Ui, theme: &Theme) -> ExtensionResult<crate::ThemeSetResult> {
        let result = self.ui_themed(resource, "setTheme", &json!(theme.key))?;
        Ok(theme_set_result(&result))
    }
    fn ui_get_tools_expanded(&self, resource: &Ui) -> ExtensionResult<bool> {
        let expanded = self.ui_themed(resource, "getToolsExpanded", &serde_json::Value::Null)?;
        Ok(expanded == true)
    }
    fn ui_set_tools_expanded(&self, resource: &Ui, expanded: bool) -> ExtensionResult<()> {
        self.ui_themed(resource, "setToolsExpanded", &json!(expanded))
            .map(drop)
    }
    fn ui_on_terminal_input(
        &self,
        resource: &Ui,
        handler: &Callback,
    ) -> ExtensionResult<Self::Subscription> {
        super::ui_host::on_terminal_input(&mut self.observed.borrow_mut(), resource.key, handler.id)
    }
    fn ui_select<'a>(
        &'a self,
        resource: &'a Ui,
        title: String,
        options: Vec<String>,
        dialog: Dialog<'a, Signal>,
    ) -> ExtensionFuture<'a, Option<String>> {
        let operands = dialog_operands(json!([title, options]), &dialog);
        Box::pin(async move {
            let decision = ui_dialog(
                &self.observed,
                resource.key,
                "select",
                &operands,
                dialog.signal,
            )
            .await?;
            Ok(super::ui_host::chosen(&decision))
        })
    }
    fn ui_confirm<'a>(
        &'a self,
        resource: &'a Ui,
        title: String,
        message: String,
        dialog: Dialog<'a, Signal>,
    ) -> ExtensionFuture<'a, bool> {
        let operands = dialog_operands(json!([title, message]), &dialog);
        Box::pin(async move {
            let decision = ui_dialog(
                &self.observed,
                resource.key,
                "confirm",
                &operands,
                dialog.signal,
            )
            .await?;
            Ok(super::ui_host::confirmed(&decision))
        })
    }
    fn ui_input<'a>(
        &'a self,
        resource: &'a Ui,
        title: String,
        placeholder: Option<String>,
        dialog: Dialog<'a, Signal>,
    ) -> ExtensionFuture<'a, Option<String>> {
        let operands = dialog_operands(json!([title, placeholder]), &dialog);
        Box::pin(async move {
            let decision = ui_dialog(
                &self.observed,
                resource.key,
                "input",
                &operands,
                dialog.signal,
            )
            .await?;
            Ok(super::ui_host::chosen(&decision))
        })
    }
    fn ui_editor<'a>(
        &'a self,
        resource: &'a Ui,
        title: String,
        prefill: Option<String>,
    ) -> ExtensionFuture<'a, Option<String>> {
        let operands = json!([title, prefill]);
        Box::pin(async move {
            let decision =
                ui_dialog(&self.observed, resource.key, "editor", &operands, None).await?;
            Ok(super::ui_host::chosen(&decision))
        })
    }
    type Registry = Reader;
    type Reader = Reader;
    type Writer = Reader;
    type Node = Node;
    type Update = Update;
    type Callback = Callback;
    type Context = Context;
    type CommandContext = CommandContext;
    type ReplacedContext = ReplacedContext;

    fn session_manager(&self, _context: &Context) -> ExtensionResult<Reader> {
        let data = self.observed.borrow().readers.acquire()?.fresh();
        Ok(Reader {
            data,
            _lease: Lease::new(&self.observed),
        })
    }
    fn command_session_manager(&self, context: &CommandContext) -> ExtensionResult<Reader> {
        let data = context
            .setup
            .clone()
            .map_or_else(|| self.observed.borrow().readers.acquire(), Ok)?;
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
        self.acquire_ui(&context.session.cwd)
    }
    fn has_ui(&self, context: &Context) -> ExtensionResult<bool> {
        let _ = context;
        self.catalog_has_ui()
    }
    fn model_registry(&self, context: &Context) -> ExtensionResult<Reader> {
        let _ = context;
        self.catalog_registry()
    }
    fn model(&self, context: &Context) -> ExtensionResult<Option<String>> {
        self.catalog_model(&context.model)
    }
    fn is_idle(&self, _: &Context) -> ExtensionResult<bool> {
        self.catalog_idle()
    }
    fn signal(&self, _: &Context) -> ExtensionResult<Option<Signal>> {
        self.catalog_signal()
    }
    fn abort(&self, _: &Context) -> ExtensionResult<()> {
        self.catalog_abort()
    }
    fn has_pending_messages(&self, _: &Context) -> ExtensionResult<bool> {
        self.catalog_pending()
    }
    fn shutdown(&self, _: &Context) -> ExtensionResult<()> {
        self.catalog_shutdown()
    }
    fn get_context_usage(&self, _: &Context) -> ExtensionResult<Option<crate::ContextUsage>> {
        self.catalog_usage()
    }
    fn get_system_prompt(&self, _: &Context) -> ExtensionResult<String> {
        self.catalog_prompt()
    }
    fn command_ui(&self, context: &CommandContext) -> ExtensionResult<Ui> {
        self.acquire_ui(&context.session.cwd)
    }
    fn command_has_ui(&self, context: &CommandContext) -> ExtensionResult<bool> {
        let _ = context;
        self.catalog_has_ui()
    }
    fn command_model_registry(&self, context: &CommandContext) -> ExtensionResult<Reader> {
        let _ = context;
        self.catalog_registry()
    }
    fn command_model(&self, context: &CommandContext) -> ExtensionResult<Option<String>> {
        self.catalog_model(&context.model)
    }
    fn command_is_idle(&self, _: &CommandContext) -> ExtensionResult<bool> {
        self.catalog_idle()
    }
    fn command_signal(&self, _: &CommandContext) -> ExtensionResult<Option<Signal>> {
        self.catalog_signal()
    }
    fn command_abort(&self, _: &CommandContext) -> ExtensionResult<()> {
        self.catalog_abort()
    }
    fn command_has_pending_messages(&self, _: &CommandContext) -> ExtensionResult<bool> {
        self.catalog_pending()
    }
    fn command_shutdown(&self, _: &CommandContext) -> ExtensionResult<()> {
        self.catalog_shutdown()
    }
    fn command_get_context_usage(
        &self,
        _: &CommandContext,
    ) -> ExtensionResult<Option<crate::ContextUsage>> {
        self.catalog_usage()
    }
    fn command_get_system_prompt(&self, _: &CommandContext) -> ExtensionResult<String> {
        self.catalog_prompt()
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
    fn registry_refresh(&self, resource: &Reader) -> ExtensionResult<()> {
        resource.data.unit("refresh", &[])
    }
    fn registry_get_error(&self, resource: &Reader) -> ExtensionResult<Option<String>> {
        resource.data.optional("getError", &[])
    }
    fn registry_has_configured_auth(
        &self,
        resource: &Reader,
        model: &str,
    ) -> ExtensionResult<bool> {
        resource
            .data
            .flag("hasConfiguredAuth", &[serde_json::json!(model)])
    }
    fn registry_get_provider_auth_status(
        &self,
        resource: &Reader,
        provider: &str,
    ) -> ExtensionResult<String> {
        resource
            .data
            .record("getProviderAuthStatus", &[serde_json::json!(provider)])?
            .ok_or("missing status".into())
    }
    fn registry_get_provider_display_name(
        &self,
        resource: &Reader,
        provider: &str,
    ) -> ExtensionResult<String> {
        resource
            .data
            .optional("getProviderDisplayName", &[serde_json::json!(provider)])?
            .ok_or("missing name".into())
    }
    fn registry_is_using_oauth(&self, resource: &Reader, model: &str) -> ExtensionResult<bool> {
        resource
            .data
            .flag("isUsingOAuth", &[serde_json::json!(model)])
    }
    fn registry_register_provider(
        &self,
        resource: &Reader,
        name: &str,
        config: &str,
        _callbacks: crate::component_adapter::ProviderRegistration<'_, Self::Callback>,
    ) -> ExtensionResult<()> {
        resource.data.unit(
            "registerProvider",
            &[serde_json::json!(name), serde_json::json!(config)],
        )
    }
    fn registry_unregister_provider(&self, resource: &Reader, name: &str) -> ExtensionResult<()> {
        resource
            .data
            .unit("unregisterProvider", &[serde_json::json!(name)])
    }
    fn registry_get_api_key_for_provider<'a>(
        &'a self,
        resource: &'a Reader,
        provider: String,
    ) -> ExtensionFuture<'a, Option<String>> {
        Box::pin(async move {
            let hold = self.observed.borrow_mut().take_hold();
            pause(hold).await;
            resource
                .data
                .optional("getApiKeyForProvider", &[serde_json::json!(provider)])
        })
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

    type Theme = Theme;
    fn theme_thinking_border(
        &self,
        theme: &Theme,
        level: &str,
        text: &str,
    ) -> ExtensionResult<String> {
        theme_text(
            &self.observed,
            theme,
            "thinkingBorder",
            &json!([level, text]),
        )
    }
    fn theme_bash_mode_border(&self, theme: &Theme, text: &str) -> ExtensionResult<String> {
        theme_text(&self.observed, theme, "bashModeBorder", &json!([text]))
    }
    fn theme_name(&self, theme: &Theme) -> ExtensionResult<Option<String>> {
        super::theme_host::optional(&mut self.observed.borrow_mut(), &theme.key, "name")
    }
    fn theme_source_path(&self, theme: &Theme) -> ExtensionResult<Option<String>> {
        super::theme_host::optional(&mut self.observed.borrow_mut(), &theme.key, "sourcePath")
    }
    fn theme_source_info(&self, theme: &Theme) -> ExtensionResult<Option<String>> {
        super::theme_host::source_info(&mut self.observed.borrow_mut(), &theme.key)
    }
    fn theme_set_source_info(&self, theme: &Theme, source: Option<&str>) -> ExtensionResult<()> {
        let operands = serde_json::json!([source]);
        super::theme_host::request(
            &mut self.observed.borrow_mut(),
            &theme.key,
            "setSourceInfo",
            &operands,
        )
        .map(drop)
    }
    fn theme_bold(&self, theme: &Theme, text: &str) -> ExtensionResult<String> {
        theme_text(&self.observed, theme, "bold", &serde_json::json!([text]))
    }
    fn theme_italic(&self, theme: &Theme, text: &str) -> ExtensionResult<String> {
        theme_text(&self.observed, theme, "italic", &serde_json::json!([text]))
    }
    fn theme_underline(&self, theme: &Theme, text: &str) -> ExtensionResult<String> {
        theme_text(
            &self.observed,
            theme,
            "underline",
            &serde_json::json!([text]),
        )
    }
    fn theme_inverse(&self, theme: &Theme, text: &str) -> ExtensionResult<String> {
        theme_text(&self.observed, theme, "inverse", &serde_json::json!([text]))
    }
    fn theme_strikethrough(&self, theme: &Theme, text: &str) -> ExtensionResult<String> {
        theme_text(
            &self.observed,
            theme,
            "strikethrough",
            &serde_json::json!([text]),
        )
    }
    fn theme_fg(&self, theme: &Theme, color: &str, text: &str) -> ExtensionResult<String> {
        theme_text(
            &self.observed,
            theme,
            "fg",
            &serde_json::json!([color, text]),
        )
    }
    fn theme_bg(&self, theme: &Theme, color: &str, text: &str) -> ExtensionResult<String> {
        theme_text(
            &self.observed,
            theme,
            "bg",
            &serde_json::json!([color, text]),
        )
    }
    fn theme_get_fg_ansi(&self, theme: &Theme, color: &str) -> ExtensionResult<String> {
        theme_text(
            &self.observed,
            theme,
            "getFgAnsi",
            &serde_json::json!([color]),
        )
    }
    fn theme_get_bg_ansi(&self, theme: &Theme, color: &str) -> ExtensionResult<String> {
        theme_text(
            &self.observed,
            theme,
            "getBgAnsi",
            &serde_json::json!([color]),
        )
    }
    fn theme_get_color_mode(&self, theme: &Theme) -> ExtensionResult<crate::ColorMode> {
        let mode = theme_text(
            &self.observed,
            theme,
            "getColorMode",
            &serde_json::json!([]),
        )?;
        Ok(if mode == "truecolor" {
            crate::ColorMode::Truecolor
        } else {
            crate::ColorMode::Color256
        })
    }
    fn register_message_renderer(
        &self,
        custom_type: &str,
        handler: &Callback,
    ) -> ExtensionResult<()> {
        let mut observed = self.observed.borrow_mut();
        if let Some(error) = observed.registrations.refusal() {
            return Err(error);
        }
        observed.register(format!("renderer {custom_type}"), handler.id);
        observed.log(format!(
            "renderer-register {}",
            serde_json::json!(custom_type)
        ));
        Ok(())
    }

    fn register_command(
        &self,
        name: &str,
        description: Option<&str>,
        completions: Option<&Callback>,
        handler: &Callback,
    ) -> ExtensionResult<()> {
        let mut observed = self.observed.borrow_mut();
        if let Some(error) = observed.registrations.refusal() {
            return Err(error);
        }
        observed.register_command(name, handler.id);
        observed.registrations.commands.push((
            name.into(),
            description.map(str::to_owned),
            completions.is_some(),
        ));
        if let Some(completions) = completions {
            observed.register(format!("completion {name}"), completions.id);
        }
        Ok(())
    }
    fn register_shortcut(
        &self,
        shortcut: &str,
        description: Option<&str>,
        handler: &Callback,
    ) -> ExtensionResult<()> {
        let mut observed = self.observed.borrow_mut();
        if let Some(error) = observed.registrations.refusal() {
            return Err(error);
        }
        observed
            .registrations
            .shortcuts
            .push((shortcut.into(), description.map(str::to_owned)));
        observed.register(format!("shortcut {shortcut}"), handler.id);
        Ok(())
    }
    fn contribute_load_time(&self, contributions: &str) -> ExtensionResult<()> {
        self.observed
            .borrow_mut()
            .log(format!("load-time {contributions}"));
        Ok(())
    }
    fn register_flag(&self, name: &str, options: &crate::FlagOptions) -> ExtensionResult<()> {
        if let Some(error) = self.observed.borrow_mut().registrations.refusal() {
            return Err(error);
        }
        self.observed
            .borrow_mut()
            .registrations
            .flags
            .push((name.into(), super::registration_host::options(options)));
        Ok(())
    }
    fn get_flag(&self, name: &str) -> ExtensionResult<Option<crate::FlagValue>> {
        self.observed.borrow().registrations.value(name)
    }

    fn register_tool(
        &self,
        metadata: &str,
        callbacks: crate::component_adapter::ToolCallbacks<'_, Callback>,
        schemas: &[(String, &Callback)],
    ) -> ExtensionResult<()> {
        let name = super::schema_host::tool_name(metadata).map_err(|error| error.to_string())?;
        {
            let mut observed = self.observed.borrow_mut();
            observed.schemas.retain(|(tool, _, _)| tool != &name);
            observed.schemas.extend(
                schemas
                    .iter()
                    .map(|(pointer, callback)| (name.clone(), pointer.clone(), callback.id)),
            );
        }
        let id = |callback: Option<&Callback>| callback.map(|callback| callback.id);
        self.observed.borrow_mut().register_tool(
            metadata,
            (id(callbacks.prepare), callbacks.execute.id),
            (id(callbacks.render_call), id(callbacks.render_result)),
        )
    }

    fn tool_update(&self, update: &Update, partial: &str) -> ExtensionResult<()> {
        let value: serde_json::Value = serde_json::from_str(partial).map_err(|e| e.to_string())?;
        update
            .observed
            .borrow_mut()
            .update(partial, &value["details"])?;
        if let Some(flag) = &update.cancel_on_first {
            flag.abort();
        }
        Ok(())
    }

    fn register_provider(
        &self,
        name: &str,
        config: &str,
        _callbacks: crate::component_adapter::ProviderRegistration<'_, Self::Callback>,
    ) -> ExtensionResult<()> {
        self.observed
            .borrow_mut()
            .log(format!("provider register {name} {config}"));
        Ok(())
    }

    fn unregister_provider(&self, name: &str) -> ExtensionResult<()> {
        self.observed
            .borrow_mut()
            .log(format!("provider unregister {name}"));
        Ok(())
    }

    fn append_entry(&self, custom_type: &str, data: Option<&str>) -> ExtensionResult<()> {
        if custom_type == "state Ω" {
            self.observed.borrow().actions.check()?;
        }
        self.observed.borrow_mut().entry(custom_type, data);
        Ok(())
    }

    fn new_signal(&self) -> ExtensionResult<Signal> {
        Ok(self.signal(&Flag::default()))
    }

    fn signal_abort(&self, signal: &Signal) -> ExtensionResult<()> {
        self.abort(&signal.flag).into_iter().collect()
    }

    fn aborted(&self, signal: &Signal) -> bool {
        signal.flag.0.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn signal_subscribe(
        &self,
        signal: &Signal,
        listener: &Callback,
    ) -> ExtensionResult<Self::Subscription> {
        self.observed
            .borrow_mut()
            .bus
            .on(&abort_channel(&signal.flag), listener.id)
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
        data: Option<NewSessionCommandData>,
        setup: Option<&'a Callback>,
        with_session: Option<&'a Callback>,
    ) -> ExtensionFuture<'a, SessionChangeResult> {
        Box::pin(async move {
            let parent = data
                .as_ref()
                .and_then(|data| data.parent_session.as_deref())
                .unwrap_or("none");
            let hold = super::setup_host::command_start(
                &mut self.observed.borrow_mut(),
                data.as_ref()
                    .and_then(|data| data.parent_session.as_deref()),
                (setup.is_some(), with_session.is_some()),
                data.is_some(),
            );
            pause(hold).await;
            if let Some(error) = self.observed.borrow().setup.session_error.clone() {
                return Err(error);
            }
            self.observed
                .borrow_mut()
                .log(format!("new-session start parent={parent}"));
            if self.observed.borrow_mut().take_rejection() {
                return Err("session rejected".to_owned());
            }
            if self.observed.borrow().setup.cancelled {
                return Ok(SessionChangeResult { cancelled: true });
            }
            if self.observed.borrow().setup.unused {
                return Ok(SessionChangeResult { cancelled: false });
            }
            let bound = setup.map(|_| super::setup_host::reader(parent));
            if let Some(callback) = setup {
                let data = bound.clone().unwrap();
                Exports::new(self.clone())
                    .invoke_setup(
                        callback.id,
                        Reader {
                            data,
                            _lease: Lease::new(&self.observed),
                        },
                    )
                    .await?;
            }
            if let Some(callback) = with_session {
                let mut replaced = self.replaced_context("/replacement");
                replaced.setup = bound;
                Exports::new(self.clone())
                    .invoke_with_session(callback.id, replaced)
                    .await?;
            }
            context.session.stale.set(true);
            self.observed
                .borrow_mut()
                .log("new-session done cancelled=false");
            Ok(SessionChangeResult { cancelled: false })
        })
    }

    fn command(&self, context: &ReplacedContext) -> CommandContext {
        let mut command = self.command_context(&context.session.cwd);
        command.setup = context.setup.clone();
        command
    }
}

impl Controlled {
    /// Constructs both custom factory forms without retaining their borrowed emitter.
    async fn custom(&self, factory: &Callback) -> ExtensionResult<()> {
        let (built, close) = self.custom_build(factory).await?;
        self.custom_input(built, &super::bus_host::Emitter { key: 0 })?;
        assert!(close.is_closed());
        Ok(())
    }
    /// Runs a custom factory's prefix, then awaits its owned continuation when there is one.
    pub(super) async fn custom_build(
        &self,
        factory: &Callback,
    ) -> ExtensionResult<(crate::ExtensionComponent, super::custom_host::Close)> {
        let close = super::custom_host::Close::default();
        let lent = crate::component_adapter::custom::Lent {
            tui: Tui::lent(&self.observed),
            theme: self.theme("dark"),
            keybindings: super::widget_controlled::Keybindings(0),
            close: close.clone(),
        };
        let emitter = super::bus_host::Emitter { key: 0 };
        let built = Exports::new(self.clone()).invoke_custom_factory(factory.id, lent, &emitter)?;
        let built = match built {
            crate::CustomFactoryOutcome::Ready(built) => built,
            crate::CustomFactoryOutcome::Async(building) => building.await?,
        };
        Ok((built, close))
    }
    /// Exercises the view after the prefix returns, then disposes it before releasing it.
    fn custom_input(
        &self,
        built: crate::ExtensionComponent,
        emitter: &super::bus_host::Emitter,
    ) -> ExtensionResult<()> {
        use super::renderer_host::{Call, Reply, invoke};
        let script = self
            .observed
            .borrow()
            .ui
            .custom_script
            .clone()
            .unwrap_or_default();
        assert!(
            script.ticks.is_empty(),
            "independent tick scripts use the component host"
        );
        assert_eq!(built.width, script.width);
        assert_eq!(script.frames.len(), script.inputs.len() + 1);
        for (index, frame) in script.frames.iter().enumerate() {
            assert_eq!(
                invoke(&built.component, Call::Render(script.columns), self, 0)?,
                Reply::Lines(frame.clone())
            );
            if let Some(data) = script.inputs.get(index) {
                invoke(&built.component, Call::Input(data), self, 0)?;
            }
        }
        if let Some(dispose) = built.dispose {
            crate::component_adapter::renderers::with_emitter(self, emitter, |emitter| {
                dispose(emitter)
            })?;
        }
        Ok(())
    }
    /// Calls the actual backend body and verifies the script's native-owner error channel.
    fn credential_backend(&self, resource: &Reader) -> ExtensionResult<()> {
        let target = resource.data.1.lock().unwrap().backend;
        if let Some(target) = target {
            let result =
                crate::component_adapter::auth_storage::invoke_backend(self, target, resource);
            let expected = resource.data.optional("backend_result", &[])?;
            assert_eq!(result.err(), expected);
        }
        Ok(())
    }
    /// Scripted metadata must agree with the callback's actual result before returning.
    fn credential_fallback(resource: &Reader, provider: &str) -> ExtensionResult<()> {
        let target = resource.data.1.lock().unwrap().fallback;
        if let Some(target) =
            target.filter(|_| super::credential_host::scripted_fallback(&resource.data, provider))
        {
            let result = crate::component_adapter::auth_storage::invoke_fallback(target, provider);
            let expected = resource
                .data
                .optional("fallback_result", &[serde_json::json!(provider)]);
            assert_eq!(result, expected);
            result?;
        }
        Ok(())
    }
}
