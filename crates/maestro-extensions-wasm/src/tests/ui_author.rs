//! Author operations for the basic retained-UI requests.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use maestro_extensions_wasm::{
    AbortController, AbortListener, AbortSignal, CallbackEmitter, CallbackEmitterPort,
    CommandOptions, CompactOptions, Component, ComponentDisposer, ComponentFactory, ComponentView,
    ExtensionAPI, ExtensionComponent, ExtensionContext, ExtensionEvent, ExtensionResult,
    ExtensionUIContext, ExtensionUIDialogOptions, ExtensionWidgetOptions, FooterData,
    FooterFactory, NotificationType, SignalPort, Subscription, TerminalInputResult, Text, Theme,
    ThemeSelection, ThemeSetResult, ToolDefinition, ToolRenderCall, ToolRenderContext,
    ToolRenderResult, WidgetPlacement, WorkingIndicatorOptions, is_focusable,
};
use serde::Deserialize;
use serde_json::{Value, json};

/// Severity written by a request.
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    /// Informational.
    Info,
    /// A warning.
    Warning,
    /// An error.
    Error,
}

impl From<Kind> for NotificationType {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::Info => Self::Info,
            Kind::Warning => Self::Warning,
            Kind::Error => Self::Error,
        }
    }
}

/// Placement written by a request.
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Placement {
    /// Above the editor.
    AboveEditor,
    /// Below the editor.
    BelowEditor,
}

impl From<Placement> for WidgetPlacement {
    fn from(placement: Placement) -> Self {
        match placement {
            Placement::AboveEditor => Self::AboveEditor,
            Placement::BelowEditor => Self::BelowEditor,
        }
    }
}

/// Supplied indicator members, each independently absent.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Indicator {
    /// Animation frames; an empty list hides the indicator.
    #[serde(default)]
    frames: Option<Vec<String>>,
    /// Frame interval in milliseconds.
    #[serde(default)]
    interval_ms: Option<f64>,
}

/// Supplied widget options.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Widget {
    /// Requested placement.
    #[serde(default)]
    placement: Option<Placement>,
}

/// One retained-UI request.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum Call {
    /// Shows a notification with an optional severity.
    Notify {
        /// Literal message.
        message: String,
        /// Optional severity.
        #[serde(default)]
        kind: Option<Kind>,
    },
    /// Sets or clears one status key.
    SetStatus {
        /// Literal key.
        key: String,
        /// Literal text, or its removal.
        #[serde(default)]
        text: Option<String>,
    },
    /// Sets the working message, or restores the host default.
    SetWorkingMessage {
        /// Literal message, or its reset.
        #[serde(default)]
        message: Option<String>,
    },
    /// Shows or hides the working loader row.
    SetWorkingVisible {
        /// Requested visibility.
        visible: bool,
    },
    /// Configures the working indicator, or restores the host default.
    SetWorkingIndicator {
        /// Supplied options, or their reset.
        #[serde(default)]
        options: Option<Indicator>,
    },
    /// Sets the hidden-thinking label, or restores the host default.
    SetHiddenThinkingLabel {
        /// Literal label, or its reset.
        #[serde(default)]
        label: Option<String>,
    },
    /// Sets the fixed lines of one widget, or removes it.
    SetWidget {
        /// Literal key.
        key: String,
        /// Ordered lines, or the removal.
        #[serde(default)]
        content: Option<Vec<String>>,
        /// Supplied options.
        #[serde(default)]
        options: Option<Widget>,
    },
    /// Sets the terminal title.
    SetTitle {
        /// Literal title.
        title: String,
    },
    /// Pastes literal text into the host editor.
    PasteToEditor {
        /// Literal pasted text.
        text: String,
    },
    /// Replaces the host editor's text.
    SetEditorText {
        /// Literal replacement text.
        text: String,
    },
    /// Reads the host editor's text.
    GetEditorText,
}

/// Records the release of a factory widget's captures through the author's API.
struct Capture(ExtensionAPI, String);

impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.0.append_entry("widget-released", Some(json!(self.1)));
    }
}

/// A factory widget that renders its label and the width it was given.
struct LabelView {
    /// Widget label.
    label: String,
    /// The theme the factory received, kept with the view.
    _theme: Theme,
    /// Released with the view.
    _capture: Capture,
}

impl ComponentView for LabelView {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        Ok(vec![format!("{} {width}", self.label)])
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}

/// A factory that logs each build and, when asked, each disposal of the widget it builds.
fn label_factory(api: &ExtensionAPI, label: String, dispose: bool) -> ComponentFactory {
    let log = api.clone();
    Rc::new(move |_tui, theme, _emitter| {
        log.append_entry("widget-factory", Some(json!(label)))?;
        let disposal = dispose.then(|| {
            let (log, label) = (log.clone(), label.clone());
            Rc::new(move |_: CallbackEmitter<'_>| {
                log.append_entry("widget-dispose", Some(json!(label)))
            }) as ComponentDisposer
        });
        let view = LabelView {
            label: label.clone(),
            _theme: theme,
            _capture: Capture(log.clone(), label.clone()),
        };
        Ok(ExtensionComponent {
            component: Component::new(view),
            dispose: disposal,
            set_expanded: None,
            width: None,
        })
    })
}

/// A footer that renders its label and the footer metadata it reads on every render.
struct FooterView {
    /// Footer label.
    label: String,
    /// The footer metadata the factory received.
    footer: FooterData,
    /// Released with the view.
    _capture: Capture,
}

impl ComponentView for FooterView {
    fn render(&self, _: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        let branch = self.footer.get_git_branch()?;
        let statuses = self.footer.get_extension_statuses()?;
        let providers = self.footer.get_available_provider_count()?;
        Ok(vec![
            json!([self.label, branch, statuses, providers]).to_string(),
        ])
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}

thread_local! {
    /// Component aliases the author keeps after their widgets were shown.
    static ALIASES: RefCell<Vec<Component>> = RefCell::default();
    /// Branch change subscriptions by footer label.
    static BRANCHES: RefCell<HashMap<String, Subscription>> = RefCell::default();
}

/// A footer factory that subscribes to branch changes and logs each build and disposal.
fn footer_factory(api: &ExtensionAPI, label: String) -> FooterFactory {
    let log = api.clone();
    Rc::new(move |_tui, _theme, footer, _emitter| {
        log.append_entry("widget-factory", Some(json!(label)))?;
        let (changed, named) = (log.clone(), label.clone());
        let subscription = footer.on_branch_change(Rc::new(move || {
            changed.append_entry("branch-changed", Some(json!(named)))
        }))?;
        BRANCHES.with_borrow_mut(|branches| branches.insert(label.clone(), subscription));
        let (disposed, named) = (log.clone(), label.clone());
        let dispose: ComponentDisposer =
            Rc::new(move |_| disposed.append_entry("widget-dispose", Some(json!(named))));
        let view = FooterView {
            label: label.clone(),
            footer,
            _capture: Capture(log.clone(), label.clone()),
        };
        Ok(ExtensionComponent {
            component: Component::new(view),
            dispose: Some(dispose),
            set_expanded: None,
            width: None,
        })
    })
}

/// Fixed lines a tool renderer returns.
struct Lines(Vec<String>);

impl ComponentView for Lines {
    fn render(&self, _: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        Ok(self.0.clone())
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}

/// Renderings of one row, counted in the row's typed state.
#[derive(Default)]
struct Renders(u32);

/// Everything a renderer read about its row, as one line, after counting the rendering in
/// the row's state and redrawing the row when its arguments ask for it.
fn painted(slot: &str, args: Option<&Value>, ctx: &ToolRenderContext) -> ExtensionResult<Value> {
    let renders = ctx.state::<Renders>();
    renders.borrow_mut().0 += 1;
    if args.is_some_and(|args| args["invalidate"] == true) {
        (ctx.invalidate)()?;
    }
    let phase = ctx.phase;
    Ok(
        json!({ "slot": slot, "args": args, "argsSeen": ctx.args, "id": ctx.tool_call_id,
        "cwd": ctx.cwd, "started": phase.execution_started, "complete": phase.args_complete,
        "partial": phase.is_partial, "expanded": ctx.expanded, "images": ctx.show_images,
        "error": ctx.is_error, "renders": renders.borrow().0,
        "last": ctx.last_component.is_some() }),
    )
}

/// A tool whose renderers report what they read about their row.
fn painter() -> ExtensionResult<ToolDefinition> {
    let metadata = serde_json::from_value(json!({ "name": "painter", "label": "Painter",
        "description": "Paints its row", "parameters": { "type": "object" } }))
    .map_err(|error| error.to_string())?;
    let render_call: ToolRenderCall = Rc::new(|args, _theme, ctx, _emitter| {
        let line = painted("call", args.as_ref(), ctx)?;
        Ok(Component::new(Lines(vec![line.to_string()])))
    });
    let render_result: ToolRenderResult = Rc::new(|result, options, _theme, ctx, _emitter| {
        let mut line = painted("result", ctx.args.as_ref(), ctx)?;
        line["result"] = json!(result);
        line["options"] = json!([options.expanded, options.is_partial]);
        Ok(Component::new(Lines(vec![line.to_string()])))
    });
    Ok(ToolDefinition {
        metadata,
        prepare_arguments: None,
        execute: Rc::new(|_, _, _, _, _| Box::pin(async { Err("not executed".to_owned()) })),
        render_call: Some(render_call),
        render_result: Some(render_result),
    })
}

/// One theme or tool expansion request.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum ThemeCall {
    /// Reads the name of the UI's live theme.
    Current,
    /// Lists every available theme.
    GetAllThemes,
    /// Loads a theme and reads its name.
    GetTheme {
        /// Literal theme name.
        name: String,
    },
    /// Selects a theme by name.
    SetThemeName {
        /// Literal theme name.
        name: String,
    },
    /// Loads a theme, then selects that retained theme.
    SetLoaded {
        /// Literal theme name.
        name: String,
    },
    /// Reads the tool output expansion.
    GetToolsExpanded,
    /// Changes the tool output expansion.
    SetToolsExpanded {
        /// Requested expansion.
        expanded: bool,
    },
}

/// A selection result as its two members.
fn selected(result: &ThemeSetResult) -> Value {
    json!({ "success": result.success, "error": result.error })
}

/// Runs one theme or tool expansion request through the supplied handle.
fn themes(ui: &ExtensionUIContext, call: ThemeCall) -> ExtensionResult<Value> {
    Ok(match call {
        ThemeCall::Current => json!(ui.theme()?.name()?),
        ThemeCall::GetAllThemes => {
            let themes = ui.get_all_themes()?.into_iter();
            json!(
                themes
                    .map(|it| json!({ "name": it.name, "path": it.path }))
                    .collect::<Vec<_>>()
            )
        }
        ThemeCall::GetTheme { name } => {
            json!(ui.get_theme(&name)?.map(|it| it.name()).transpose()?)
        }
        ThemeCall::SetThemeName { name } => selected(&ui.set_theme(ThemeSelection::Name(name))?),
        ThemeCall::SetLoaded { name } => {
            let theme = ui.get_theme(&name)?.ok_or("no theme to select")?;
            selected(&ui.set_theme(ThemeSelection::Theme(theme))?)
        }
        ThemeCall::GetToolsExpanded => json!(ui.get_tools_expanded()?),
        ThemeCall::SetToolsExpanded { expanded } => {
            ui.set_tools_expanded(expanded)?;
            Value::Null
        }
    })
}

/// Which caller signal a dialog receives.
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CallerSignal {
    /// A new signal, kept after the dialog.
    Live,
    /// A new signal aborted before the dialog, kept after it.
    Aborted,
    /// A new signal aborted while the dialog is pending, kept after it.
    AbortPending,
    /// The kept signal of this index, shared by an earlier dialog.
    Kept(usize),
}

/// Supplied dialog options.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Opts {
    /// Caller milliseconds.
    #[serde(default)]
    timeout: Option<f64>,
    /// Caller signal.
    #[serde(default)]
    signal: Option<CallerSignal>,
}

/// One dialog request.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
pub enum Dialog {
    /// Shows a selector.
    Select {
        /// Literal title.
        title: String,
        /// Ordered literal choices.
        options: Vec<String>,
        /// Supplied options.
        #[serde(default)]
        opts: Option<Opts>,
    },
    /// Asks for confirmation.
    Confirm {
        /// Literal title.
        title: String,
        /// Literal message.
        message: String,
        /// Supplied options.
        #[serde(default)]
        opts: Option<Opts>,
    },
    /// Reads one line.
    Input {
        /// Literal title.
        title: String,
        /// Literal placeholder, or none.
        #[serde(default)]
        placeholder: Option<String>,
        /// Supplied options.
        #[serde(default)]
        opts: Option<Opts>,
    },
    /// Reads several lines.
    Editor {
        /// Literal title.
        title: String,
        /// Literal prefilled text, or none.
        #[serde(default)]
        prefill: Option<String>,
    },
}

/// A public controller with a capture count witnessing dialog listener cleanup.
#[derive(Default)]
pub struct Controller {
    /// The caller-owned cancellation used by the dialog.
    inner: AbortController,
    /// One extra strong reference per live listener capture.
    captures: Rc<()>,
}

/// The caller signal port tracking the lifetime of each delegated listener.
struct Caller(Rc<Controller>);

/// Emissions from unrelated authored UI callbacks are discarded.
struct Silent;

impl CallbackEmitterPort for Silent {
    fn emit(&self, _channel: &str, _data: Value) -> ExtensionResult<()> {
        Ok(())
    }
}

impl SignalPort for Caller {
    fn aborted(&self) -> bool {
        self.0.inner.signal().aborted()
    }
    fn subscribe(&self, listener: AbortListener) -> ExtensionResult<Subscription> {
        let capture = Rc::clone(&self.0.captures);
        self.0.inner.signal().subscribe(Rc::new(move |emitter| {
            let _ = &capture;
            listener(emitter)
        }))
    }
}

impl Controller {
    /// Aborts through the public caller-owned controller.
    fn abort(&self) -> ExtensionResult<()> {
        self.inner.abort()
    }
}

thread_local! {
    /// Caller signals kept after their dialogs, by creation order.
    static KEPT: RefCell<Vec<Rc<Controller>>> = RefCell::default();
    /// Terminal input subscriptions by label.
    static LISTENING: RefCell<HashMap<String, Subscription>> = RefCell::default();
}

/// The decision the listener `label` makes about the chunk `data`.
fn terminal_decision(label: &str, data: &str) -> ExtensionResult<Option<TerminalInputResult>> {
    let decision = |consume, data| Some(TerminalInputResult { consume, data });
    Ok(match data {
        "none" => None,
        "absent" => decision(None, None),
        "empty" => decision(None, Some(String::new())),
        "consume" => decision(Some(true), None),
        "pass" => decision(Some(false), None),
        "both" => decision(Some(true), Some(format!("{label} both"))),
        "fail" => return Err(format!("{label} failed 雪\nhere")),
        _ => decision(None, Some(format!("{label} {data}"))),
    })
}

/// Adds the terminal input listener `label` through a fresh acquisition.
fn listen(ctx: &ExtensionContext, label: String) -> ExtensionResult<Value> {
    let named = label.clone();
    let subscription = ctx.ui()?.on_terminal_input(Rc::new(move |data, emitter| {
        emitter.emit("terminal-input", json!([named, data]))?;
        terminal_decision(&named, data)
    }))?;
    LISTENING.with_borrow_mut(|listening| listening.insert(label, subscription));
    Ok(json!("listening"))
}

/// Unsubscribes the listener `label`, or drops its subscription without unsubscribing.
fn stop_listening(label: &str, unsubscribe: bool) -> ExtensionResult<Value> {
    let subscription = LISTENING.with_borrow_mut(|listening| listening.remove(label));
    let subscription = subscription.ok_or("unknown listener")?;
    if unsubscribe {
        subscription.unsubscribe()?;
        return Ok(json!("unsubscribed"));
    }
    Ok(json!("dropped"))
}

/// Aborts the kept caller signal `index` and reports how many listeners it still had.
fn abort_kept(index: usize) -> ExtensionResult<Value> {
    let controller = KEPT.with_borrow(|kept| kept.get(index).cloned());
    let controller = controller.ok_or("unknown kept signal")?;
    let listeners = Rc::strong_count(&controller.captures) - 1;
    controller.abort()?;
    Ok(json!({ "listeners": listeners }))
}

/// The options a dialog receives, and the controller to abort while it is pending.
fn options(
    opts: Option<Opts>,
) -> ExtensionResult<(Option<ExtensionUIDialogOptions>, Option<Rc<Controller>>)> {
    let Some(opts) = opts else {
        return Ok((None, None));
    };
    let controller = match opts.signal {
        None => None,
        Some(CallerSignal::Kept(index)) => Some(
            KEPT.with_borrow(|kept| kept.get(index).cloned())
                .ok_or("unknown kept signal")?,
        ),
        Some(_) => {
            let controller = Rc::new(Controller::default());
            KEPT.with_borrow_mut(|kept| kept.push(Rc::clone(&controller)));
            Some(controller)
        }
    };
    if matches!(opts.signal, Some(CallerSignal::Aborted)) {
        controller
            .iter()
            .try_for_each(|controller| controller.abort())?;
    }
    let pending = controller
        .clone()
        .filter(|_| matches!(opts.signal, Some(CallerSignal::AbortPending)));
    let signal = controller.map(|controller| AbortSignal::new(Rc::new(Caller(controller))));
    let options = ExtensionUIDialogOptions {
        signal,
        timeout: opts.timeout,
    };
    Ok((Some(options), pending))
}

/// Awaits `answer`, aborting `pending` after its first poll leaves it waiting on the host.
async fn settled<T>(
    mut answer: maestro_extensions_wasm::ExtensionFuture<'static, T>,
    pending: Option<Rc<Controller>>,
) -> ExtensionResult<T> {
    if let Some(controller) = pending {
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        if let std::task::Poll::Ready(result) = answer.as_mut().poll(&mut context) {
            return result;
        }
        controller.abort()?;
    }
    answer.await
}

/// Runs one dialog through the supplied handle and returns its answer.
pub async fn ask(ui: &ExtensionUIContext, dialog: Dialog) -> ExtensionResult<Value> {
    Ok(match dialog {
        Dialog::Select {
            title,
            options: choices,
            opts,
        } => {
            let (opts, pending) = options(opts)?;
            json!(settled(ui.select(title, choices, opts), pending).await?)
        }
        Dialog::Confirm {
            title,
            message,
            opts,
        } => {
            let (opts, pending) = options(opts)?;
            json!(settled(ui.confirm(title, message, opts), pending).await?)
        }
        Dialog::Input {
            title,
            placeholder,
            opts,
        } => {
            let (opts, pending) = options(opts)?;
            json!(settled(ui.input(title, placeholder, opts), pending).await?)
        }
        Dialog::Editor { title, prefill } => json!(ui.editor(title, prefill).await?),
    })
}

/// One probe operation around a fresh acquisition and its callbacks.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
enum Operation {
    /// Performs a request through a freshly acquired handle.
    Acquire {
        /// The request.
        call: Call,
    },
    /// Shows a dialog through a freshly acquired handle.
    Dialog {
        /// The dialog.
        call: Dialog,
    },
    /// Runs a theme or tool expansion request through a fresh acquisition.
    Theme {
        /// The request.
        call: ThemeCall,
    },
    /// Shows a labelled factory widget through a fresh acquisition.
    WidgetFactory {
        /// Widget key.
        key: String,
        /// Label the widget renders.
        label: String,
        /// Requested placement.
        #[serde(default)]
        placement: Option<Placement>,
        /// Whether the widget has a disposal.
        #[serde(default)]
        dispose: bool,
    },
    /// Replaces the footer with a labelled factory component, or restores the host's footer.
    Footer {
        /// Label the footer renders; absent restores the host's footer.
        #[serde(default)]
        label: Option<String>,
    },
    /// Registers the painter tool, whose renderers report what they read.
    Painter,
    /// Shows a component with optional width and expansion.
    FactoryCapabilities {
        /// The supplied width.
        width: Option<f64>,
        /// Whether the component accepts expansion.
        expandable: bool,
    },
    /// Shows an authored editor through a retained component.
    AuthoredEditorContract,
    /// Stacks a completion provider named `name` on the current provider.
    StackProvider {
        /// The candidate the provider adds.
        name: String,
    },
    /// Replaces the host editor with the editor `kind` names, or restores it for `none`.
    EditorFactory {
        /// `plain`, `host`, `custom`, `failing` or `none`.
        kind: String,
    },
    /// Calls an authored asynchronous completion provider.
    CompletionContract {
        /// Whether completion was explicitly requested.
        force: Option<bool>,
    },
    /// Installs a retained terminal view.
    TuiFactory,
    /// Checks the authored component focus contract.
    FocusContract,
    /// Checks host editors through the component focus capability.
    HostEditorFocus,
    /// Reads and replaces one retained overlay options owner.
    OverlayOptions,
    /// Runs an immediately ready or asynchronous custom factory.
    Custom {
        /// Whether the factory returns a future.
        asynchronous: bool,
    },
    /// Shows a custom overlay with static or resolved placement.
    CustomOverlay {
        /// Whether a resolver supplies the placement.
        resolved: bool,
    },
    /// Shows a widget that drives the frame writer its factory received.
    WriterFactory,
    /// Drains the input of the last driver widget's terminal.
    WriterDrain,
    /// Runs a host widget script.
    Widgets {
        /// The steps, in order.
        steps: Vec<super::widget_author::Step>,
    },

    /// Shows a labelled factory widget whose component the author also keeps.
    KeptWidget {
        /// Label the widget renders.
        label: String,
    },
    /// Renders every kept component alias at a width.
    RenderKept {
        /// Rendering width.
        width: u32,
    },
    /// Drops every kept component alias.
    DropKept,
    /// Unsubscribes the branch change listener of a footer.
    Unbranch {
        /// Footer label.
        label: String,
    },
    /// Replaces the header with a labelled factory component, or restores the host's header.
    Header {
        /// Label the header renders; absent restores the host's header.
        #[serde(default)]
        label: Option<String>,
    },
    /// Adds a labelled terminal input listener through a fresh acquisition.
    Listen {
        /// Listener label.
        label: String,
    },
    /// Unsubscribes a labelled terminal input listener.
    Unlisten {
        /// Listener label.
        label: String,
    },
    /// Drops a labelled terminal input subscription without unsubscribing.
    Forget {
        /// Listener label.
        label: String,
    },
    /// Aborts a caller signal kept after its dialog.
    AbortKept {
        /// Creation index of the kept signal.
        index: usize,
    },
    /// Registers the bus listener through one acquisition, which reports its owner once.
    Observe,
    /// Starts one compaction whose callbacks report through one acquisition, which reports its
    /// owner once.
    Compact,
    /// Emits one supplied payload on a supplied channel.
    Emit {
        /// Literal channel.
        channel: String,
        /// Payload to emit.
        input: Value,
    },
    /// Supplies the host rejection of the next request, the host's next UI binding or other
    /// host state, forwarded as given.
    Control(serde_json::Map<String, Value>),
}

/// Runs one request through the supplied handle, returning what a read request answered.
pub fn perform(ui: &ExtensionUIContext, call: &Call) -> ExtensionResult<Option<Value>> {
    match call {
        Call::GetEditorText => return ui.get_editor_text().map(|text| Some(json!(text))),
        Call::PasteToEditor { text } => ui.paste_to_editor(text),
        Call::SetEditorText { text } => ui.set_editor_text(text),
        Call::Notify { message, kind } => ui.notify(message, kind.map(Into::into)),
        Call::SetStatus { key, text } => ui.set_status(key, text.as_deref()),
        Call::SetWorkingMessage { message } => ui.set_working_message(message.as_deref()),
        Call::SetWorkingVisible { visible } => ui.set_working_visible(*visible),
        Call::SetWorkingIndicator { options } => {
            ui.set_working_indicator(options.as_ref().map(|options| WorkingIndicatorOptions {
                frames: options.frames.clone(),
                interval_ms: options.interval_ms,
            }))
        }
        Call::SetHiddenThinkingLabel { label } => ui.set_hidden_thinking_label(label.as_deref()),
        Call::SetWidget {
            key,
            content,
            options,
        } => ui.set_widget(
            key,
            content.as_deref(),
            options.as_ref().map(|options| ExtensionWidgetOptions {
                placement: options.placement.map(Into::into),
            }),
        ),
        Call::SetTitle { title } => ui.set_title(title),
    }
    .map(|()| None)
}

/// Reports a caught host failure without failing the callback.
fn caught(result: ExtensionResult<Option<Value>>) -> Value {
    match result {
        Ok(answer) => json!({ "ok": answer.unwrap_or(json!(true)) }),
        Err(error) => json!({ "error": error }),
    }
}

/// Checks authored focus without assuming every component has that capability.
fn focus_probe(ctx: &ExtensionContext) -> ExtensionResult<Option<Value>> {
    super::focus_author::install(&ctx.ui()?)?;
    let emitter = CallbackEmitter::new(&Silent);
    let field = super::focus_author::component();
    let plain = Component::from(&Text::new("plain", 0, 0, None)?);
    assert!(!is_focusable(Some(&plain)));
    assert_eq!(plain.focused(emitter)?, None);
    plain.set_focused(true, emitter)?;
    assert_eq!(plain.focused(emitter)?, None);
    assert!(!is_focusable(None));
    assert!(is_focusable(Some(&field)));
    assert_eq!(field.focused(emitter)?, Some(false));
    field.set_focused(true, emitter)?;
    assert_eq!(field.focused(emitter)?, Some(true));
    field.set_focused(false, emitter)?;
    field.set_focused(true, emitter)?;
    Ok(Some(json!(field.render(10, emitter)?)))
}

/// Drops the author's retained aliases without removing the host's components.
fn drop_kept() -> Value {
    drop(ALIASES.take());
    json!("dropped")
}

/// Renders every kept component alias with a silent emitter.
fn render_kept(width: u32) -> ExtensionResult<Value> {
    let aliases = ALIASES.with_borrow(Clone::clone);
    let rendered = aliases
        .iter()
        .map(|alias| alias.render(width, CallbackEmitter::new(&Silent)))
        .collect::<ExtensionResult<Vec<_>>>()?;
    Ok(json!(rendered))
}

/// Shows a factory widget, header or footer, or unsubscribes a footer's branch listener.
fn factories(
    api: &ExtensionAPI,
    ctx: &ExtensionContext,
    operation: Operation,
) -> ExtensionResult<()> {
    match operation {
        Operation::TuiFactory => super::tui_author::install(&ctx.ui()?),
        Operation::WriterFactory => super::writer_author::install(&ctx.ui()?),
        Operation::HostEditorFocus => super::focus_author::install_editors(&ctx.ui()?),
        Operation::WidgetFactory {
            key,
            label,
            placement,
            dispose,
        } => {
            let options = placement.map(|placement| ExtensionWidgetOptions {
                placement: Some(placement.into()),
            });
            let factory = label_factory(api, label, dispose);
            ctx.ui()?.set_widget_factory(&key, factory, options)
        }
        Operation::FactoryCapabilities { width, expandable } => {
            super::factory_component_author::install(&ctx.ui()?, width, expandable)
        }
        Operation::Footer { label } => {
            let factory = label.map(|label| footer_factory(api, label));
            ctx.ui()?.set_footer(factory)
        }
        Operation::KeptWidget { label } => {
            let inner = label_factory(api, label, true);
            let factory: ComponentFactory = Rc::new(move |tui, theme, emitter| {
                let built = inner(tui, theme, emitter)?;
                ALIASES.with_borrow_mut(|aliases| aliases.push(built.component.clone()));
                Ok(built)
            });
            ctx.ui()?.set_widget_factory("w", factory, None)
        }
        Operation::Unbranch { label } => {
            let subscription = BRANCHES.with_borrow_mut(|branches| branches.remove(&label));
            subscription.map_or(Ok(()), |subscription| subscription.unsubscribe())
        }
        Operation::Header { label } => {
            let factory = label.map(|label| label_factory(api, label, true));
            ctx.ui()?.set_header(factory)
        }
        _ => Err("expected a factory operation".into()),
    }
}

/// Runs a custom component, reads and replaces retained overlay placement, or drains the
/// frame writer's terminal.
async fn custom(ctx: &ExtensionContext, operation: Operation) -> ExtensionResult<String> {
    match operation {
        Operation::OverlayOptions => super::overlay_author::options(),
        Operation::WriterDrain => super::writer_author::drain().await,
        Operation::Custom { asynchronous } => {
            super::custom_author::run(&ctx.ui()?, asynchronous).await
        }
        Operation::CustomOverlay { resolved } => {
            super::custom_overlay_author::run(&ctx.ui()?, resolved).await
        }
        _ => Err("expected a custom component operation".into()),
    }
}

/// Runs one operation and reports its outcome.
async fn dispatch(api: &ExtensionAPI, ctx: &ExtensionContext, text: &str) -> ExtensionResult<()> {
    let operation: Operation = serde_json::from_str(text).map_err(|error| error.to_string())?;
    let answer = match operation {
        Operation::Acquire { call } => caught(perform(&ctx.ui()?, &call)),
        Operation::Dialog { call } => caught(ask(&ctx.ui()?, call).await.map(Some)),
        Operation::AbortKept { index } => caught(abort_kept(index).map(Some)),
        Operation::Theme { call } => caught(themes(&ctx.ui()?, call).map(Some)),
        Operation::RenderKept { width } => caught(render_kept(width).map(Some)),
        Operation::Painter => caught(
            painter()
                .and_then(|tool| api.register_tool(tool))
                .map(|()| None),
        ),
        operation @ (Operation::AuthoredEditorContract
        | Operation::EditorFactory { .. }
        | Operation::StackProvider { .. }) => caught(editors(ctx, operation)),
        Operation::CompletionContract { force } => {
            let signal = AbortSignal::new(Rc::new(Caller(Rc::default())));
            caught(
                super::completion_author::run(ctx.ui()?, signal, force)
                    .await
                    .map(Some),
            )
        }
        operation @ (Operation::OverlayOptions
        | Operation::Custom { .. }
        | Operation::CustomOverlay { .. }
        | Operation::WriterDrain) => {
            caught(custom(ctx, operation).await.map(|value| Some(json!(value))))
        }
        Operation::FocusContract => caught(focus_probe(ctx)),
        Operation::Widgets { steps } => super::widget_author::run(steps, ctx.ui().ok()),
        Operation::DropKept => drop_kept(),
        operation @ (Operation::HostEditorFocus
        | Operation::TuiFactory
        | Operation::WriterFactory
        | Operation::WidgetFactory { .. }
        | Operation::FactoryCapabilities { .. }
        | Operation::KeptWidget { .. }
        | Operation::Footer { .. }
        | Operation::Unbranch { .. }
        | Operation::Header { .. }) => caught(factories(api, ctx, operation).map(|()| None)),
        Operation::Listen { label } => caught(listen(ctx, label).map(Some)),
        Operation::Unlisten { label } => caught(stop_listening(&label, true).map(Some)),
        Operation::Forget { label } => caught(stop_listening(&label, false).map(Some)),
        Operation::Observe => observe(api, ctx)?,
        Operation::Compact => compact(api, ctx)?,
        Operation::Emit { channel, input } => {
            api.events().emit(&channel, input).await?;
            json!("emitted")
        }
        Operation::Control(control) => {
            api.append_entry("ui-control", Some(Value::Object(control)))?;
            json!("controlled")
        }
    };
    api.append_entry("ui-answer", Some(answer))?;
    api.append_entry("ui-done", None)
}

/// Shows an authored editor through a component, installs an editor factory or stacks a
/// completion provider.
fn editors(ctx: &ExtensionContext, operation: Operation) -> ExtensionResult<Option<Value>> {
    match operation {
        Operation::EditorFactory { kind } => {
            super::editor_factory_author::install(&ctx.ui()?, &kind).map(Some)
        }
        Operation::StackProvider { name } => {
            super::stacked_provider_author::install(&ctx.ui()?, &name).map(|()| None)
        }
        _ => super::authored_editor_author::install(&ctx.ui()?).map(|()| None),
    }
}

/// Registers the bus listener through an acquired handle, which reports that owner.
fn observe(api: &ExtensionAPI, ctx: &ExtensionContext) -> ExtensionResult<Value> {
    let listener = ctx.ui()?;
    listener.notify("observe", Some(NotificationType::Info))?;
    let log = api.clone();
    api.events().on(
        "ui-bus",
        Rc::new(move |data, _| {
            listener.notify(&format!("bus {data}"), Some(NotificationType::Warning))?;
            log.append_entry("ui-bus-done", Some(data))?;
            Ok(None)
        }),
    )?;
    Ok(json!("observed"))
}

/// Starts one compaction whose callbacks report through an acquired handle, which reports that
/// owner.
fn compact(api: &ExtensionAPI, ctx: &ExtensionContext) -> ExtensionResult<Value> {
    let ui = ctx.ui()?;
    ui.notify("compact", None)?;
    let log = api.clone();
    let complete = ui.clone();
    ctx.compact(Some(CompactOptions {
        custom_instructions: None,
        on_complete: Some(Rc::new(move |result, _| {
            complete.notify(&format!("complete {}", result.summary), None)?;
            log.append_entry("ui-complete-done", Some(json!(result.summary)))
        })),
        on_error: Some(Rc::new(move |message, _| {
            ui.notify(&format!("error {message}"), Some(NotificationType::Error))
        })),
    }))?;
    Ok(json!("compacted"))
}

/// Registers the event handler and the command that share these requests.
pub fn register(api: &ExtensionAPI) -> ExtensionResult<()> {
    let event_api = api.clone();
    api.on(
        "ui",
        Rc::new(move |event, ctx| {
            let api = event_api.clone();
            Box::pin(async move {
                let ExtensionEvent::Input(input) = event else {
                    return Err("expected input".into());
                };
                dispatch(&api, &ctx, &input.text).await?;
                Ok(None)
            })
        }),
    )?;
    let log = api.clone();
    api.register_command(
        "ui",
        CommandOptions {
            get_argument_completions: None,
            description: None,
            handler: Rc::new(move |args, ctx| {
                let log = log.clone();
                Box::pin(async move { dispatch(&log, &ctx, &args).await })
            }),
        },
    )
}
