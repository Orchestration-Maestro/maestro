//! Host decisions and literal retained-UI requests shared by both hosts.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::collections::HashMap;
use std::str::FromStr;

use serde::Deserialize;
use serde_json::{Value, json};

use super::host::{Flag, State};
use super::observed::Observed;
use crate::bindings::host_side::maestro::extension::host;
use wasmtime::component::{Accessor, Resource};

/// Host type of the retained UI capability.
pub struct Ui(pub(super) usize);

/// Host state the UI requests are served from.
#[derive(Default)]
pub struct Bank {
    /// Identity of the binding acquisitions currently resolve.
    binding: usize,
    /// Rejection supplied for the next request it applies to.
    error: Option<Refusal>,
    /// Method whose next request is rejected instead of the next request overall.
    method: Option<String>,
    /// Message the request of `method` is rejected with.
    message: Option<String>,
    /// Editor text each binding keeps.
    editors: HashMap<usize, String>,
    /// Decision the next dialog answers with; absent dismisses it.
    answer: Option<Value>,
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
    /// Decision the next dialog answers with; absent dismisses it.
    answer: Option<Value>,
}

impl Bank {
    /// Mutate the host before acknowledging the control entry.
    pub fn control(&mut self, text: &str) {
        let control: Control = serde_json::from_str(text).unwrap();
        if let Some(binding) = control.binding {
            self.binding = binding;
        }
        self.answer = control.answer;
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

    /// Hands out the identity of the binding current at acquisition time.
    pub fn acquire(&self) -> usize {
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

/// Records one replacement against the acquisition `key` and keeps its text for that binding.
pub fn set_editor_text(observed: &mut Observed, key: usize, text: &str) -> Result<(), String> {
    call(observed, key, "setEditorText", &json!([text]))?;
    observed.ui.editors.insert(key, text.to_owned());
    Ok(())
}

/// Records one read against the acquisition `key` and answers the text its binding keeps.
pub fn get_editor_text(observed: &mut Observed, key: usize) -> Result<String, String> {
    call(observed, key, "getEditorText", &json!([]))?;
    Ok(observed.ui.editors.get(&key).cloned().unwrap_or_default())
}

/// Records whether the signal of a settling dialog on the acquisition `key` is aborted.
pub fn settle(observed: &mut Observed, key: usize, aborted: &Value) {
    observed.log(format!("ui {key} settle {}", json!([aborted])));
}

/// The bus channel of the terminal input listeners added through the acquisition `key`.
pub fn terminal_channel(key: usize) -> String {
    format!("terminal {key}")
}

/// Records one terminal input listener added through the acquisition `key`, then registers
/// it on that acquisition's channel unless the host refuses it.
pub fn on_terminal_input(
    observed: &mut Observed,
    key: usize,
    handler: u32,
) -> Result<super::bus_host::Subscription, String> {
    call(observed, key, "onTerminalInput", &json!([]))?;
    observed.bus.on(&terminal_channel(key), handler)
}

/// The terminal input listeners in registration order, across acquisitions.
pub fn terminal_listeners(observed: &Observed) -> Vec<u32> {
    observed
        .bus
        .listeners
        .iter()
        .filter(|(channel, _)| channel.starts_with("terminal "))
        .map(|(_, id)| *id)
        .collect()
}

/// A terminal input listener's consumption and replacement members.
pub type Decision = (Option<bool>, Option<String>);

/// One listener's answer: its decision, with absent members kept absent, or its failure.
pub fn terminal_answer(answer: Result<Option<Decision>, String>) -> Value {
    match answer {
        Ok(decision) => json!({
            "ok": decision.map(|(consume, data)| json!({ "consume": consume, "data": data }))
        }),
        Err(error) => json!({ "error": error }),
    }
}

/// The decision supplied for the dialog that is settling.
pub fn decision(observed: &mut Observed) -> Value {
    observed.ui.answer.take().unwrap_or(Value::Null)
}

/// The text a dialog decision chooses; any other decision dismisses the dialog.
pub fn chosen(decision: &Value) -> Option<String> {
    decision.as_str().map(str::to_owned)
}

/// Whether a decision confirms; any other decision is a dismissal.
pub fn confirmed(decision: &Value) -> bool {
    decision.as_bool().unwrap_or(false)
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

/// One retained-resource request a host recorded; `K` keys the resource, by default the
/// identity of a UI acquisition.
pub struct Receipt<K = usize> {
    /// Key of the resource the request was made on.
    pub key: K,
    /// Method the host reached.
    pub method: String,
    /// Operands the host received.
    pub operands: Value,
}

/// The request `line` records on a `kind` resource, its key read as `K`, if it records one.
fn receipt_on<K: FromStr>(kind: &str, line: &str) -> Option<Receipt<K>> {
    let rest = line.strip_prefix(kind)?.strip_prefix(' ')?;
    let (key, rest) = rest.split_once(' ')?;
    let (method, operands) = rest.split_once(' ')?;
    Some(Receipt {
        key: key.parse().ok()?,
        method: method.to_owned(),
        operands: serde_json::from_str(operands).ok()?,
    })
}

/// The requests `lines` record on `kind` resources, in time order.
pub fn receipts_on<K: FromStr>(kind: &str, lines: &[String]) -> Vec<Receipt<K>> {
    lines
        .iter()
        .filter_map(|line| receipt_on(kind, line))
        .collect()
}

/// The UI request `line` records, if it records one.
pub fn receipt(line: &str) -> Option<Receipt> {
    receipt_on("ui", line)
}

/// The UI requests recorded in `lines`, in time order.
pub fn receipts(lines: &[String]) -> Vec<Receipt> {
    receipts_on("ui", lines)
}

impl State {
    /// Whether a lent dialog signal is aborted, or null without one.
    fn flag_state(&self, signal: Option<&Resource<Flag>>) -> wasmtime::Result<Value> {
        signal.map_or(Ok(Value::Null), |signal| {
            let flag = &self.table.get(signal)?.0;
            Ok(json!(flag.load(std::sync::atomic::Ordering::SeqCst)))
        })
    }

    /// Records one dialog made through `this`, returning its supplied rejection.
    fn ui_dialog(
        &mut self,
        this: &Resource<Ui>,
        method: &str,
        operands: &Value,
    ) -> wasmtime::Result<Result<(), String>> {
        let key = self.table.get(this)?.0;
        Ok(call(&mut self.observed, key, method, operands))
    }

    /// Records one retained UI request made through `this`.
    fn ui_call(
        &mut self,
        this: &Resource<Ui>,
        method: &str,
        operands: &Value,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        let key = self.table.get(this)?.0;
        let result = call(&mut self.observed, key, method, operands);
        self.routed(result)
    }
}

impl host::HostExtensionUiContext for State {
    fn notify(
        &mut self,
        this: Resource<Ui>,
        message: String,
        kind: Option<host::NotificationType>,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        let seen = kind.map(variant);
        self.ui_call(&this, "notify", &notify(&message, seen.as_deref()))
    }

    fn set_status(
        &mut self,
        this: Resource<Ui>,
        key: String,
        text: Option<String>,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        self.ui_call(&this, "setStatus", &status(&key, text.as_deref()))
    }

    fn set_working_message(
        &mut self,
        this: Resource<Ui>,
        message: Option<String>,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
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
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        self.ui_call(&this, "setWorkingVisible", &working_visible(visible))
    }

    fn set_working_indicator(
        &mut self,
        this: Resource<Ui>,
        options: Option<host::WorkingIndicatorOptions>,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
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
    ) -> wasmtime::Result<Resource<super::host::Route>> {
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
    ) -> wasmtime::Result<Resource<super::host::Route>> {
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
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        self.ui_call(&this, "setTitle", &self::title(&title))
    }

    fn paste_to_editor(
        &mut self,
        this: Resource<Ui>,
        text: String,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        self.ui_call(&this, "pasteToEditor", &json!([text]))
    }

    fn set_editor_text(
        &mut self,
        this: Resource<Ui>,
        text: String,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        let key = self.table.get(&this)?.0;
        let result = set_editor_text(&mut self.observed, key, &text);
        self.routed(result)
    }

    fn on_terminal_input(
        &mut self,
        this: Resource<Ui>,
        handler: Resource<super::host::Identity>,
    ) -> wasmtime::Result<Result<Resource<super::bus_host::Subscription>, String>> {
        let key = self.table.get(&this)?.0;
        match on_terminal_input(&mut self.observed, key, handler.rep()) {
            Ok(subscription) => Ok(Ok(self.table.push(subscription)?)),
            Err(error) => Ok(Err(error)),
        }
    }

    fn get_editor_text(&mut self, this: Resource<Ui>) -> wasmtime::Result<Result<String, String>> {
        let key = self.table.get(&this)?.0;
        Ok(get_editor_text(&mut self.observed, key))
    }

    fn drop(&mut self, this: Resource<Ui>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

/// Waits while the host holds a recorded dialog pending, records whether its signal is
/// aborted by then, and answers its decision.
async fn settled(
    accessor: &Accessor<State, super::host::Imports>,
    recorded: wasmtime::Result<Result<(), String>>,
    this: &Resource<Ui>,
    signal: Option<&Resource<Flag>>,
) -> wasmtime::Result<Result<Value, String>> {
    if let Err(error) = recorded? {
        return Ok(Err(error));
    }
    let hold = accessor.with(|mut access| access.get().observed.take_hold());
    super::observed::pause(hold).await;
    accessor.with(|mut access| {
        let state = access.get();
        if signal.is_some() {
            let key = state.table.get(this)?.0;
            let aborted = state.flag_state(signal)?;
            settle(&mut state.observed, key, &aborted);
        }
        Ok(Ok(decision(&mut state.observed)))
    })
}

impl host::HostExtensionUiContextWithStore<State> for super::host::Imports {
    async fn select(
        accessor: &Accessor<State, Self>,
        this: Resource<Ui>,
        title: String,
        options: Vec<String>,
        signal: Option<Resource<Flag>>,
        timeout: Option<f64>,
    ) -> wasmtime::Result<Result<Option<String>, String>> {
        let recorded = accessor.with(|mut access| {
            let state = access.get();
            let signal = state.flag_state(signal.as_ref())?;
            let operands = json!([title, options, signal, timeout]);
            state.ui_dialog(&this, "select", &operands)
        });
        let answer = settled(accessor, recorded, &this, signal.as_ref()).await?;
        Ok(answer.map(|decision| chosen(&decision)))
    }

    async fn confirm(
        accessor: &Accessor<State, Self>,
        this: Resource<Ui>,
        title: String,
        message: String,
        signal: Option<Resource<Flag>>,
        timeout: Option<f64>,
    ) -> wasmtime::Result<Result<bool, String>> {
        let recorded = accessor.with(|mut access| {
            let state = access.get();
            let signal = state.flag_state(signal.as_ref())?;
            let operands = json!([title, message, signal, timeout]);
            state.ui_dialog(&this, "confirm", &operands)
        });
        let answer = settled(accessor, recorded, &this, signal.as_ref()).await?;
        Ok(answer.map(|decision| confirmed(&decision)))
    }

    async fn input(
        accessor: &Accessor<State, Self>,
        this: Resource<Ui>,
        title: String,
        placeholder: Option<String>,
        signal: Option<Resource<Flag>>,
        timeout: Option<f64>,
    ) -> wasmtime::Result<Result<Option<String>, String>> {
        let recorded = accessor.with(|mut access| {
            let state = access.get();
            let signal = state.flag_state(signal.as_ref())?;
            let operands = json!([title, placeholder, signal, timeout]);
            state.ui_dialog(&this, "input", &operands)
        });
        let answer = settled(accessor, recorded, &this, signal.as_ref()).await?;
        Ok(answer.map(|decision| chosen(&decision)))
    }

    async fn editor(
        accessor: &Accessor<State, Self>,
        this: Resource<Ui>,
        title: String,
        prefill: Option<String>,
    ) -> wasmtime::Result<Result<Option<String>, String>> {
        let operands = json!([title, prefill]);
        let recorded =
            accessor.with(|mut access| access.get().ui_dialog(&this, "editor", &operands));
        let answer = settled(accessor, recorded, &this, None).await?;
        Ok(answer.map(|decision| chosen(&decision)))
    }
}
