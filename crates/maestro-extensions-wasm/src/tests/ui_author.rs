//! Author operations for the basic retained-UI requests.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use maestro_extensions_wasm::{
    AbortListener, AbortSignal, CallbackEmitter, CallbackEmitterPort, CommandOptions,
    CompactOptions, ExtensionAPI, ExtensionContext, ExtensionEvent, ExtensionResult,
    ExtensionUIContext, ExtensionUIDialogOptions, ExtensionWidgetOptions, NotificationType,
    SignalPort, Subscription, SubscriptionPort, WidgetPlacement, WorkingIndicatorOptions,
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

/// A caller-owned cancellation whose listeners run when it aborts.
#[derive(Default)]
pub struct Controller {
    /// Whether it was aborted.
    aborted: Cell<bool>,
    /// Live listeners by registration.
    listeners: RefCell<Vec<(usize, AbortListener)>>,
    /// Next registration.
    next: Cell<usize>,
}

/// The caller signal port over a shared controller.
struct Caller(Rc<Controller>);

/// Removes one listener from its controller.
struct Removal(Weak<Controller>, usize);

/// Emissions of an authored abort are discarded.
struct Silent;

impl CallbackEmitterPort for Silent {
    fn emit(&self, _channel: &str, _data: Value) -> ExtensionResult<()> {
        Ok(())
    }
}

impl SubscriptionPort for Removal {
    fn unsubscribe(&self) -> ExtensionResult<()> {
        if let Some(controller) = self.0.upgrade() {
            controller
                .listeners
                .borrow_mut()
                .retain(|(id, _)| *id != self.1);
        }
        Ok(())
    }
}

impl SignalPort for Caller {
    fn aborted(&self) -> bool {
        self.0.aborted.get()
    }
    fn subscribe(&self, listener: AbortListener) -> ExtensionResult<Subscription> {
        let id = self.0.next.replace(self.0.next.get() + 1);
        self.0.listeners.borrow_mut().push((id, listener));
        Ok(Subscription::new(Rc::new(Removal(
            Rc::downgrade(&self.0),
            id,
        ))))
    }
}

impl Controller {
    /// Aborts once and runs the listeners registered at that moment.
    fn abort(&self) -> ExtensionResult<()> {
        if self.aborted.replace(true) {
            return Ok(());
        }
        let listeners: Vec<AbortListener> = self
            .listeners
            .borrow()
            .iter()
            .map(|(_, listener)| Rc::clone(listener))
            .collect();
        for listener in listeners {
            listener(CallbackEmitter::new(&Silent))?;
        }
        Ok(())
    }
}

thread_local! {
    /// Caller signals kept after their dialogs, by creation order.
    static KEPT: RefCell<Vec<Rc<Controller>>> = RefCell::default();
}

/// Aborts the kept caller signal `index` and reports how many listeners it still had.
fn abort_kept(index: usize) -> ExtensionResult<Value> {
    let controller = KEPT.with_borrow(|kept| kept.get(index).cloned());
    let controller = controller.ok_or("unknown kept signal")?;
    let listeners = controller.listeners.borrow().len();
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
    /// Supplies the host rejection of the next request, or the host's next UI binding.
    Control {
        /// Message the next request is rejected with.
        #[serde(default)]
        error: Option<String>,
        /// Identity of the binding later acquisitions resolve.
        #[serde(default)]
        binding: Option<usize>,
        /// Decision the host's next dialog answers with.
        #[serde(default)]
        answer: Option<Value>,
    },
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

/// Runs one operation and reports its outcome.
async fn dispatch(api: &ExtensionAPI, ctx: &ExtensionContext, text: &str) -> ExtensionResult<()> {
    let operation: Operation = serde_json::from_str(text).map_err(|error| error.to_string())?;
    let answer = match operation {
        Operation::Acquire { call } => caught(perform(&ctx.ui()?, &call)),
        Operation::Dialog { call } => caught(ask(&ctx.ui()?, call).await.map(Some)),
        Operation::AbortKept { index } => caught(abort_kept(index).map(Some)),
        Operation::Observe => observe(api, ctx)?,
        Operation::Compact => compact(api, ctx)?,
        Operation::Emit { channel, input } => {
            api.events().emit(&channel, input).await?;
            json!("emitted")
        }
        Operation::Control {
            error,
            binding,
            answer,
        } => {
            api.append_entry(
                "ui-control",
                Some(json!({ "error": error, "binding": binding, "answer": answer })),
            )?;
            json!("controlled")
        }
    };
    api.append_entry("ui-answer", Some(answer))?;
    api.append_entry("ui-done", None)
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
