//! Author operations for the basic retained-UI requests.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

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

/// One probe operation around a fresh acquisition and its callbacks.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "camelCase")]
enum Operation {
    /// Performs a request through a freshly acquired handle.
    Acquire {
        /// The request.
        call: Call,
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
        Operation::Observe => observe(api, ctx)?,
        Operation::Compact => compact(api, ctx)?,
        Operation::Emit { channel, input } => {
            api.events().emit(&channel, input).await?;
            json!("emitted")
        }
        Operation::Control { error, binding } => {
            api.append_entry(
                "ui-control",
                Some(json!({ "error": error, "binding": binding })),
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
