//! Author operations for the basic retained-UI requests.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::cell::RefCell;
use std::rc::Rc;

use maestro_extensions_wasm::{
    CommandOptions, CompactOptions, ExtensionAPI, ExtensionContext, ExtensionEvent,
    ExtensionResult, ExtensionUIContext, ExtensionWidgetOptions, NotificationType, WidgetPlacement,
    WorkingIndicatorOptions,
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
}

/// One probe operation around a request and the retained handles.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
enum Operation {
    /// Retains the UI of the current context.
    Retain,
    /// Retains another alias of the first retained handle.
    Alias,
    /// Drops the last retained handle.
    Pop,
    /// Registers the retained listener and starts a compaction.
    Observe,
    /// Emits one supplied payload on a supplied channel.
    Emit {
        /// Literal channel.
        channel: String,
        /// Payload to emit.
        input: Value,
    },
    /// Performs a request through a freshly acquired handle.
    Acquire {
        /// The request.
        call: Call,
    },
    /// Performs a request through a retained handle.
    Retained {
        /// Retained handle to use.
        #[serde(default)]
        index: usize,
        /// The request.
        call: Call,
    },
    /// Supplies the host rejection of the next request.
    Control {
        /// Message the next request is rejected with.
        error: String,
    },
}

/// Independently retained handles.
#[derive(Default)]
struct Kept {
    /// Retained UI owners.
    uis: Vec<ExtensionUIContext>,
}

impl Kept {
    /// The retained handle at `index`.
    fn at(&self, index: usize) -> ExtensionResult<ExtensionUIContext> {
        self.uis
            .get(index)
            .cloned()
            .ok_or_else(|| "missing UI".to_owned())
    }
}

/// Runs one request through the supplied handle.
pub fn perform(ui: &ExtensionUIContext, call: &Call) -> ExtensionResult<()> {
    match call {
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
}

/// Reports a caught host failure without failing the callback.
fn caught(result: ExtensionResult<()>) -> Value {
    match result {
        Ok(()) => json!({ "ok": true }),
        Err(error) => json!({ "error": error }),
    }
}

/// Runs one operation against the supplied context and reports its outcome.
async fn dispatch(
    kept: &Rc<RefCell<Kept>>,
    api: &ExtensionAPI,
    ctx: &ExtensionContext,
    text: &str,
) -> ExtensionResult<()> {
    let operation: Operation = serde_json::from_str(text).map_err(|error| error.to_string())?;
    let answer = match operation {
        Operation::Retain => {
            kept.borrow_mut().uis.push(ctx.ui()?);
            json!("retained")
        }
        Operation::Alias => {
            let alias = kept.borrow().at(0)?;
            kept.borrow_mut().uis.push(alias);
            json!("retained")
        }
        Operation::Pop => {
            let dropped = kept.borrow_mut().uis.pop();
            drop(dropped);
            json!("dropped")
        }
        Operation::Observe => observe(kept, api, ctx)?,
        Operation::Emit { channel, input } => {
            api.events().emit(&channel, input).await?;
            json!("emitted")
        }
        Operation::Acquire { call } => caught(perform(&ctx.ui()?, &call)),
        Operation::Retained { index, call } => caught(perform(&kept.borrow().at(index)?, &call)),
        Operation::Control { error } => {
            api.append_entry("ui-control", Some(json!({ "error": error })))?;
            json!("controlled")
        }
    };
    api.append_entry("ui-answer", Some(answer))?;
    api.append_entry("ui-done", None)
}

/// Registers the retained listener and starts a compaction, both reporting through the first
/// retained handle.
fn observe(
    kept: &Rc<RefCell<Kept>>,
    api: &ExtensionAPI,
    ctx: &ExtensionContext,
) -> ExtensionResult<Value> {
    let ui = kept.borrow().at(0)?;
    let log = api.clone();
    let listener = ui.clone();
    api.events().on(
        "ui-bus",
        Rc::new(move |data, _| {
            listener.notify(&format!("bus {data}"), Some(NotificationType::Warning))?;
            log.append_entry("ui-bus-done", Some(data))?;
            Ok(None)
        }),
    )?;
    let log = api.clone();
    let complete = ui.clone();
    let error = ui;
    ctx.compact(Some(CompactOptions {
        custom_instructions: None,
        on_complete: Some(Rc::new(move |result, _| {
            complete.notify(&format!("complete {}", result.summary), None)?;
            log.append_entry("ui-complete-done", Some(json!(result.summary)))
        })),
        on_error: Some(Rc::new(move |message, _| {
            error.notify(&format!("error {message}"), Some(NotificationType::Error))
        })),
    }))?;
    Ok(json!("observed"))
}

/// Registers the event handler and the command that share these requests.
pub fn register(api: &ExtensionAPI) -> ExtensionResult<()> {
    let kept = Rc::new(RefCell::new(Kept::default()));
    let event_kept = Rc::clone(&kept);
    let event_api = api.clone();
    api.on(
        "ui",
        Rc::new(move |event, ctx| {
            let kept = Rc::clone(&event_kept);
            let api = event_api.clone();
            Box::pin(async move {
                let ExtensionEvent::Input(input) = event else {
                    return Err("expected input".into());
                };
                dispatch(&kept, &api, &ctx, &input.text).await?;
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
                let kept = Rc::clone(&kept);
                let log = log.clone();
                Box::pin(async move { dispatch(&kept, &log, &ctx, &args).await })
            }),
        },
    )
}
