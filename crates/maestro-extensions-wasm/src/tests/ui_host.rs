//! Host decisions and literal retained-UI requests shared by both hosts.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use serde::Deserialize;
use serde_json::{Value, json};

use super::host::State;
use super::observed::Observed;
use crate::bindings::host_side::maestro::extension::host;
use wasmtime::component::Resource;

/// Host type of the retained UI capability.
pub struct Ui(pub(super) usize);

/// Host state the UI requests are served from.
#[derive(Default)]
pub struct Bank {
    /// Identity of the binding acquisitions currently resolve.
    binding: usize,
    /// Working directory of the context each acquisition was made through, in order.
    pub acquired: Vec<String>,
    /// Rejection supplied for the next request it applies to.
    error: Option<Refusal>,
    /// Method whose next request is rejected instead of the next request overall.
    method: Option<String>,
    /// Message the request of `method` is rejected with.
    message: Option<String>,
}

/// A supplied refusal and the method it is limited to.
struct Refusal {
    /// Method the refusal applies to; absent refuses the next request.
    method: Option<String>,
    /// Message the host refuses the request with.
    message: String,
}

/// Requested host state; every member is independent.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Control {
    /// Message the next request it applies to is refused with.
    error: Option<String>,
    /// Method the refusal is limited to; absent refuses the next request.
    method: Option<String>,
    /// Identity of the binding later acquisitions resolve.
    binding: Option<usize>,
}

impl Bank {
    /// Mutate the host before acknowledging the control entry.
    pub fn control(&mut self, text: &str) {
        let control: Control = serde_json::from_str(text).unwrap();
        if let Some(binding) = control.binding {
            self.binding = binding;
        }
        self.error = control.error.map(|message| Refusal {
            method: control.method,
            message,
        });
    }

    /// Takes the refusal supplied for `method`, keeping one supplied for another method.
    fn refusal(&mut self, method: &str) -> Option<String> {
        let supplied = self
            .error
            .as_ref()
            .is_some_and(|refusal| refusal.method.as_deref().is_none_or(|it| it == method));
        if supplied {
            return self.error.take().map(|refusal| refusal.message);
        }
        if self.method.as_deref() != Some(method) {
            return None;
        }
        self.method = None;
        self.message.take()
    }

    /// Hands out the identity of the binding current at acquisition time and records the
    /// working directory of the context the acquisition was made through.
    pub fn acquire(&mut self, owner: &str) -> usize {
        self.acquired.push(owner.into());
        self.binding
    }

    /// Rejects the next request of `method` with `message`, leaving other requests accepted.
    pub fn reject(&mut self, method: &str, message: &str) {
        self.method = Some(method.to_owned());
        self.message = Some(message.to_owned());
    }
}

/// Records one request against the acquisition `key`, then returns its supplied rejection.
pub fn call(
    observed: &mut Observed,
    key: usize,
    method: &str,
    operands: &Value,
) -> Result<(), String> {
    observed.log(format!("ui {key} {method} {operands}"));
    observed.ui.refusal(method).map_or(Ok(()), Err)
}

/// Supplied widget options; an absent record keeps the host default.
#[derive(Default)]
pub struct WidgetOptions {
    /// Requested placement.
    pub placement: Option<String>,
}

/// Supplied indicator options; an absent member keeps the host default.
#[derive(Default)]
pub struct IndicatorOptions {
    /// Animation frames.
    pub frames: Option<Vec<String>>,
    /// Frame interval in milliseconds.
    pub interval_ms: Option<f64>,
}

/// Literal name of a generated variant value, without its type path.
pub fn variant<T: std::fmt::Debug>(value: T) -> String {
    let debug = format!("{value:?}");
    debug.rsplit("::").next().unwrap_or_default().to_owned()
}

/// Operands of one notification request.
pub fn notify(message: &str, kind: Option<&str>) -> Value {
    json!([message, kind])
}

/// Operands of one status request.
pub fn status(key: &str, text: Option<&str>) -> Value {
    json!([key, text])
}

/// Operands of the working-message request.
pub fn working_message(message: Option<&str>) -> Value {
    json!([message])
}

/// Operands of the working-visibility request.
pub fn working_visible(visible: bool) -> Value {
    json!([visible])
}

/// Operands of the working-indicator request.
pub fn indicator(options: Option<IndicatorOptions>) -> Value {
    json!([
        options.map(|options| json!({"frames": options.frames, "intervalMs": options.interval_ms}))
    ])
}

/// Operands of the hidden-thinking-label request.
pub fn hidden_thinking_label(label: Option<&str>) -> Value {
    json!([label])
}

/// Operands of the widget request.
pub fn widget(key: &str, content: Option<&[String]>, options: Option<WidgetOptions>) -> Value {
    json!([
        key,
        content,
        options.map(|options| json!({"placement": options.placement}))
    ])
}

/// Operands of the title request.
pub fn title(title: &str) -> Value {
    json!([title])
}

/// One UI request the host recorded.
pub struct Receipt {
    /// Identity of the acquisition the request was made on.
    pub key: usize,
    /// Method the host reached.
    pub method: String,
    /// Operands the host received.
    pub operands: Value,
}

/// The UI request `line` records, if it records one.
pub fn receipt(line: &str) -> Option<Receipt> {
    let rest = line.strip_prefix("ui ")?;
    let (key, rest) = rest.split_once(' ')?;
    let (method, operands) = rest.split_once(' ')?;
    Some(Receipt {
        key: key.parse().ok()?,
        method: method.to_owned(),
        operands: serde_json::from_str(operands).ok()?,
    })
}

/// The UI requests recorded in `lines`, in time order.
pub fn receipts(lines: &[String]) -> Vec<Receipt> {
    lines.iter().filter_map(|line| receipt(line)).collect()
}

impl State {
    /// Records one retained UI request made through `this`.
    fn ui_call(
        &mut self,
        this: &Resource<Ui>,
        method: &str,
        operands: &Value,
    ) -> wasmtime::Result<Result<(), String>> {
        let key = self.table.get(this)?.0;
        Ok(call(&mut self.observed, key, method, operands))
    }
}

impl host::HostExtensionUiContext for State {
    fn notify(
        &mut self,
        this: Resource<Ui>,
        message: String,
        kind: Option<host::NotificationType>,
    ) -> wasmtime::Result<Result<(), String>> {
        let seen = kind.map(variant);
        self.ui_call(&this, "notify", &notify(&message, seen.as_deref()))
    }

    fn set_status(
        &mut self,
        this: Resource<Ui>,
        key: String,
        text: Option<String>,
    ) -> wasmtime::Result<Result<(), String>> {
        self.ui_call(&this, "setStatus", &status(&key, text.as_deref()))
    }

    fn set_working_message(
        &mut self,
        this: Resource<Ui>,
        message: Option<String>,
    ) -> wasmtime::Result<Result<(), String>> {
        self.ui_call(
            &this,
            "setWorkingMessage",
            &working_message(message.as_deref()),
        )
    }

    fn set_working_visible(
        &mut self,
        this: Resource<Ui>,
        visible: bool,
    ) -> wasmtime::Result<Result<(), String>> {
        self.ui_call(&this, "setWorkingVisible", &working_visible(visible))
    }

    fn set_working_indicator(
        &mut self,
        this: Resource<Ui>,
        options: Option<host::WorkingIndicatorOptions>,
    ) -> wasmtime::Result<Result<(), String>> {
        let options = options.map(|options| IndicatorOptions {
            frames: options.frames,
            interval_ms: options.interval_ms,
        });
        self.ui_call(&this, "setWorkingIndicator", &indicator(options))
    }

    fn set_hidden_thinking_label(
        &mut self,
        this: Resource<Ui>,
        label: Option<String>,
    ) -> wasmtime::Result<Result<(), String>> {
        self.ui_call(
            &this,
            "setHiddenThinkingLabel",
            &hidden_thinking_label(label.as_deref()),
        )
    }

    fn set_widget(
        &mut self,
        this: Resource<Ui>,
        key: String,
        content: Option<Vec<String>>,
        options: Option<host::ExtensionWidgetOptions>,
    ) -> wasmtime::Result<Result<(), String>> {
        let options = options.map(|options| WidgetOptions {
            placement: options.placement.map(variant),
        });
        self.ui_call(
            &this,
            "setWidget",
            &widget(&key, content.as_deref(), options),
        )
    }

    fn set_title(
        &mut self,
        this: Resource<Ui>,
        title: String,
    ) -> wasmtime::Result<Result<(), String>> {
        self.ui_call(&this, "setTitle", &self::title(&title))
    }

    fn drop(&mut self, this: Resource<Ui>) -> wasmtime::Result<()> {
        self.release(this)
    }
}
