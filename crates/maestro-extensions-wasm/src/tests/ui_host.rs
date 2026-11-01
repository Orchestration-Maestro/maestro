//! Host decisions and literal retained-UI requests shared by both hosts.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::collections::{HashMap, VecDeque};
use std::str::FromStr;

use serde::Deserialize;
use serde_json::{Value, json};

use super::host::{Flag, State};
use super::observed::Observed;
use super::renderer_host::Theme;
use crate::bindings::host_side::maestro::extension::host;
use wasmtime::component::{Accessor, Resource};

/// Host type of the retained UI capability.
pub struct Ui(pub(super) usize);

/// Host type of the terminal lent to a component factory.
pub struct Tui;

/// Host type of the footer metadata lent to a footer factory.
pub struct FooterResource;

/// Host type of one tool row's redraw capability.
pub struct Invalidator(pub String);

/// The footer the host keeps and the metadata a footer reads.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Footer {
    /// The custom footer the host shows, when there is one.
    #[serde(skip)]
    pub custom: Option<WidgetSlot>,
    /// The identity of the factory that built the custom footer.
    #[serde(skip)]
    pub factory: Option<u32>,
    /// Current branch.
    #[serde(default)]
    branch: Option<String>,
    /// Extension statuses in insertion order.
    #[serde(default)]
    statuses: Vec<(String, String)>,
    /// Providers with available models.
    #[serde(default)]
    providers: f64,
}

impl Footer {
    /// Answers one footer metadata read from the current metadata.
    pub fn read(&self, method: &str) -> Value {
        match method {
            "getGitBranch" => json!(self.branch),
            "getExtensionStatuses" => json!(self.statuses),
            _ => json!(self.providers),
        }
    }
}

/// One factory widget the host shows.
pub struct WidgetSlot {
    /// Widget key.
    pub key: String,
    /// Host handle of the built component.
    pub component: u32,
    /// Identity of the component's disposal, when it has one.
    pub dispose: Option<u32>,
    /// Identity of the retained expansion handler.
    pub set_expanded: Option<u32>,
    /// The supplied implicit width.
    pub width: Option<f64>,
}

/// The editor an authored factory built, as the host shows it in place of its own.
pub struct EditorSlot {
    /// Identity of the factory.
    pub factory: u32,
    /// Host handle of the editor's component.
    pub component: u32,
    /// Host handle of the editor's operations.
    pub editor: u32,
    /// Key of the custom editor widget the editor extends.
    pub custom: Option<u32>,
}

/// The header container the host keeps once its built-in header exists.
#[derive(Default)]
pub struct Header {
    /// Children in order: `banner`, `built-in` or `custom`.
    pub children: Vec<String>,
    /// Whether the built-in header exists.
    pub ready: bool,
    /// The custom header the host shows, when there is one.
    pub custom: Option<WidgetSlot>,
}

impl Header {
    /// Shows `custom`, or restores the built-in header, at the current header's slot; a custom
    /// header with no slot to take goes first.
    pub fn install(&mut self, custom: Option<WidgetSlot>) {
        let label = |custom: bool| if custom { "custom" } else { "built-in" };
        let current = label(self.custom.is_some());
        let next = label(custom.is_some()).to_owned();
        match self.children.iter().position(|child| child == current) {
            Some(index) => self.children[index] = next,
            None if custom.is_some() => self.children.insert(0, next),
            None => {}
        }
        self.custom = custom;
    }
}

/// What the result of a nested call the host made settles.
pub enum Nested {
    /// An authored component's invalidation.
    Invalidate,
    /// An authored widget callback of the list under this key; a submenu's answer opens at
    /// the setting index.
    Widget(u32, Option<usize>),
    /// The scripted native caller's backend outcome, separate from accepted state.
    CredentialBackend(Option<String>),
    /// The exact result of the author's synchronous fallback.
    CredentialFallback(Result<Option<String>, String>),
    /// A footer factory, whose component replaces the current footer.
    Footer,
    /// A header factory, whose component replaces the current header.
    Header,
    /// A widget's disposal.
    Dispose,
    /// A widget factory, whose component the host shows under this key.
    Factory(String),
    /// The interceptor of an input of the editor under this key; input it did not consume
    /// continues.
    Shortcut(u32, String),
    /// An editor factory with this identity, whose editor replaces the current one.
    Editor(u32),
    /// A provider factory, whose provider the host stacks.
    Provider,
}

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
    /// Rules a test arms for requests of one invocation.
    rules: Rules,
    /// Editor text each binding keeps.
    editors: HashMap<usize, String>,
    /// Decision the next dialog answers with; absent dismisses it.
    answer: Option<Value>,
    /// Ordered replies for a command that opens several dialogs.
    answers: VecDeque<Value>,
    /// Recorded palette the UI's live theme reads; absent reads the dark palette.
    theme: Option<String>,
    /// Supplied custom-view inputs and expected frames.
    pub custom_script: Option<super::custom_host::Script>,
    /// Factory widgets in the order they were shown.
    pub widgets: Vec<WidgetSlot>,
    /// The header container.
    pub header: Header,
    /// The footer and its metadata.
    pub footer: Footer,
    /// Whether tool output is expanded.
    tools: bool,
    /// Host widgets the extension built.
    pub hosted: super::widget_host::Bank,
    /// The lists and loaders among them.
    pub lists: super::widget_lists::Lists,
    /// Keybindings managers.
    pub keys: super::widget_keys::Managers,
    /// The editors among the widgets.
    pub widget_editors: super::widget_editors::Editors,
    /// The authored editor shown in place of the host's own, when there is one.
    pub editor: Option<EditorSlot>,
    /// The authored completion providers stacked on the host's own.
    pub providers: super::provider_host::Stack,
    /// The editor factory another extension set.
    pub foreign: super::foreign_host::Foreign,
}

/// The themes the host lists, in its order; one has no file.
pub const THEMES: [(&str, Option<&str>); 3] = [
    ("alternate", Some("/authored/theme.json")),
    ("dark", Some("/themes/dark.json")),
    ("plain", None),
];

/// Records one theme request against the acquisition `key`, then answers it from the host's
/// state: the live palette, the listed themes, a loaded palette, a selection result or the
/// tool expansion.
pub fn themed(
    observed: &mut Observed,
    key: usize,
    method: &str,
    operand: &Value,
) -> Result<Value, String> {
    call(observed, key, method, &json!([operand]))?;
    let known = |name: &str| super::theme_host::corpus().palettes.contains_key(name);
    let ui = &mut observed.ui;
    Ok(match method {
        "theme" => json!(ui.theme.clone().unwrap_or_else(|| "dark".to_owned())),
        "getAllThemes" => json!(THEMES),
        "getTheme" => json!(operand.as_str().filter(|name| known(name))),
        "setThemeName" | "setTheme" => match operand.as_str().filter(|name| known(name)) {
            Some(name) => {
                ui.theme = Some(name.to_owned());
                json!([true, null])
            }
            None => json!([
                false,
                format!("Theme not found: {}", operand.as_str().unwrap_or_default())
            ]),
        },
        "getToolsExpanded" => json!(ui.tools),
        _ => {
            ui.tools = operand.as_bool().unwrap_or_default();
            Value::Null
        }
    })
}

/// One-shot request rules a test arms directly, without a control document.
#[derive(Default)]
struct Rules {
    /// Accepted requests before the next one is rejected, counted from the arming point.
    reject_after: Option<usize>,
    /// Identity of the binding that follows the next accepted request.
    switch_after: Option<usize>,
    /// Requests the host has accepted since the rule was armed.
    accepted: usize,
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
    /// Ordered replies for successive dialogs; absent keeps the single-answer route.
    #[serde(default)]
    answers: VecDeque<Value>,
    /// Builds the built-in header: `ready` adds it after a banner, `detached` keeps only the
    /// banner.
    header: Option<String>,
    /// Replaces the footer metadata.
    footer: Option<Footer>,
    /// Makes another extension's editor factory current, `custom` or `plain`.
    foreign: Option<String>,
}

impl Bank {
    /// Mutate the host before acknowledging the control entry.
    pub fn control(&mut self, text: &str) {
        let control: Control = serde_json::from_str(text).unwrap();
        if let Some(binding) = control.binding {
            self.binding = binding;
        }
        if control.foreign.is_some() {
            self.foreign.kind = control.foreign;
        }
        self.answer = control.answer;
        self.answers = control.answers;
        if let Some(footer) = control.footer {
            let (custom, factory) = (self.footer.custom.take(), self.footer.factory.take());
            self.footer = Footer {
                custom,
                factory,
                ..footer
            };
        }
        if let Some(header) = control.header {
            let mut children = vec!["banner".to_owned()];
            children.extend((header == "ready").then(|| "built-in".to_owned()));
            self.header = Header {
                children,
                ready: true,
                custom: None,
            };
        }
        self.error = control.error.map(|message| Refusal {
            method: control.method,
            message,
        });
    }

    /// Takes a refusal supplied for exactly `method`, keeping one armed for every method.
    fn refusal(&mut self, method: &str) -> Option<String> {
        let supplied = self
            .error
            .as_ref()
            .is_some_and(|refusal| refusal.method.as_deref() == Some(method));
        if supplied {
            return self.error.take().map(|refusal| refusal.message);
        }
        if self.method.as_deref() != Some(method) {
            return None;
        }
        self.method = None;
        self.message.take()
    }

    /// Restores the state a fresh host starts from.
    pub fn reset(&mut self) {
        self.binding = 0;
        self.error = None;
        self.rules = Rules::default();
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

    /// Rejects the next request after `index` accepted ones with `error`, once.
    pub fn reject_at(&mut self, index: usize, error: &str) {
        self.rules.reject_after = Some(index);
        self.rules.accepted = 0;
        self.error = Some(Refusal {
            method: None,
            message: error.to_owned(),
        });
    }

    /// Resolves the binding `binding` for acquisitions after the next accepted request.
    pub fn switch_after_first(&mut self, binding: usize) {
        self.rules.switch_after = Some(binding);
    }

    /// Applies the armed rules to the request recorded next, taking a refusal armed for every method.
    fn settle(&mut self) -> Result<(), String> {
        let reject = self
            .rules
            .reject_after
            .is_none_or(|index| index == self.rules.accepted);
        let general = self.error.as_ref().is_some_and(|it| it.method.is_none());
        if reject
            && general
            && let Some(refusal) = self.error.take()
        {
            self.rules.reject_after = None;
            return Err(refusal.message);
        }
        self.rules.accepted += 1;
        if let Some(binding) = self.rules.switch_after.take() {
            self.binding = binding;
        }
        Ok(())
    }
}

/// Records one request against the acquisition `key`, then returns its supplied rejection.
pub fn call(
    observed: &mut Observed,
    key: usize,
    method: &str,
    operands: &Value,
) -> Result<(), String> {
    let captured = observed
        .stdout
        .as_ref()
        .map(|stdout| stdout.contents().len());
    if let Some(captured) = captured {
        observed.log(format!("captured {captured}"));
    }
    observed.log(format!("ui {key} {method} {operands}"));
    let error = observed.ui.settle().err();
    let error = error.or_else(|| observed.ui.refusal(method));
    error.map_or(Ok(()), Err)
}

/// Records one replacement against the acquisition `key` and keeps its text for that binding.
pub fn set_editor_text(observed: &mut Observed, key: usize, text: &str) -> Result<(), String> {
    call(observed, key, "setEditorText", &json!([text]))?;
    observed.ui.editors.insert(key, text.to_owned());
    Ok(())
}

/// Records an editor factory request against the acquisition `key` and removes the current
/// authored editor, returning it; the host builds the new one afterwards.
pub fn set_editor_component(
    observed: &mut Observed,
    key: usize,
    factory: Option<u32>,
) -> Result<Option<EditorSlot>, String> {
    call(
        observed,
        key,
        "setEditorComponent",
        &json!([factory.is_some()]),
    )?;
    observed.ui.foreign.kind = None;
    Ok(observed.ui.editor.take())
}

/// Records the editor a factory built and the actions of the custom editor it extends.
pub fn show_editor(observed: &mut Observed, slot: EditorSlot) {
    let actions = slot.custom.map(|custom| {
        super::widget_editors::editor(observed, custom).map(|editor| {
            editor
                .actions
                .iter()
                .map(|(action, _)| action.clone())
                .collect::<Vec<_>>()
        })
    });
    let actions = json!(actions.map(Result::unwrap_or_default));
    observed.log(format!("editor built custom={actions}"));
    observed.ui.editor = Some(slot);
}

/// Records a factory read against the acquisition `key` and answers the current factory: an
/// authored one by identity, or another extension's with whether it builds a custom editor.
pub fn get_editor_component(
    observed: &mut Observed,
    key: usize,
) -> Result<Option<Result<u32, bool>>, String> {
    call(observed, key, "getEditorComponent", &json!([]))?;
    let authored = observed.ui.editor.as_ref().map(|slot| slot.factory);
    Ok(authored
        .map(Ok)
        .or_else(|| super::foreign_host::current(observed).map(Err)))
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

/// The listeners registered on `channel`, in registration order.
pub fn listeners_on(observed: &Observed, channel: &str) -> Vec<u32> {
    observed
        .bus
        .listeners
        .iter()
        .filter(|(it, _)| it == channel)
        .map(|(_, id)| *id)
        .collect()
}

/// The text of one info member.
fn text(info: &Value, key: &str) -> String {
    info[key].as_str().unwrap_or_default().to_owned()
}

/// The info record a tool renderer reads, from its JSON description.
pub fn render_info(info: &Value) -> host::ToolRenderInfo {
    let flag = |key: &str| info[key] == true;
    host::ToolRenderInfo {
        tool_call_id: text(info, "toolCallId"),
        args: (!info["args"].is_null()).then(|| info["args"].to_string()),
        cwd: text(info, "cwd"),
        phase: host::ToolRenderPhase {
            execution_started: flag("executionStarted"),
            args_complete: flag("argsComplete"),
            is_partial: flag("isPartial"),
        },
        expanded: flag("expanded"),
        show_images: flag("showImages"),
        is_error: flag("isError"),
    }
}

/// One listener's unit answer, or its failure.
pub fn unit_answer(answer: Result<(), String>) -> Value {
    match answer {
        Ok(()) => json!({ "ok": null }),
        Err(error) => json!({ "error": error }),
    }
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

/// A selection result as success and its optional message.
pub fn set_result(result: &Value) -> host::ThemeSetResult {
    host::ThemeSetResult {
        success: result[0] == true,
        error: result[1].as_str().map(str::to_owned),
    }
}

/// The decision supplied for the dialog that is settling.
pub fn decision(observed: &mut Observed) -> Value {
    observed
        .ui
        .answers
        .pop_front()
        .or_else(|| observed.ui.answer.take())
        .unwrap_or(Value::Null)
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
    /// Removes every widget under `key` from both placements, queues each component for
    /// release and returns the disposals the guest runs first.
    fn take_widgets(
        &mut self,
        key: &str,
    ) -> wasmtime::Result<std::collections::VecDeque<(host::LocalCall, Nested)>> {
        let (removed, kept) = std::mem::take(&mut self.observed.ui.widgets)
            .into_iter()
            .partition(|slot| slot.key == key);
        self.observed.ui.widgets = kept;
        let mut steps = std::collections::VecDeque::new();
        for slot in removed {
            if let Some(handler) = slot.set_expanded {
                self.observed
                    .register(format!("expansion {handler}"), handler);
            }
            self.observed
                .register(format!("widget {}", slot.key), slot.component);
            steps.extend(self.dispose_step(slot.dispose)?);
        }
        Ok(steps)
    }

    /// The nested call of the factory under `factory`, with a fresh terminal, the dark theme and,
    /// for a footer, the footer metadata.
    fn local_factory(
        &mut self,
        factory: &Resource<super::host::Identity>,
        footer: bool,
    ) -> wasmtime::Result<host::LocalFactory> {
        Ok(host::LocalFactory {
            target: self.table.get(factory)?.0,
            tui: self.push(Tui)?,
            theme: self.push(Theme("dark".to_owned()))?,
            footer: footer.then(|| self.push(FooterResource)).transpose()?,
            emitter: self.table.push(super::bus_host::Emitter { key: 0 })?,
        })
    }

    /// The nested disposal call of `dispose`, when the component has one.
    fn dispose_step(
        &mut self,
        dispose: Option<u32>,
    ) -> wasmtime::Result<Option<(host::LocalCall, Nested)>> {
        let Some(target) = dispose else {
            return Ok(None);
        };
        let dispose = host::LocalSynchronous {
            target,
            kind: host::LocalKind::Dispose,
            data: String::new(),
            emitter: self.table.push(super::bus_host::Emitter { key: 0 })?,
        };
        Ok(Some((
            host::LocalCall::Synchronous(dispose),
            Nested::Dispose,
        )))
    }

    /// Queues the custom header for release and returns its disposal, which the guest runs
    /// before the replacement is built; the header stays shown until it is replaced.
    fn take_header(
        &mut self,
    ) -> wasmtime::Result<std::collections::VecDeque<(host::LocalCall, Nested)>> {
        let mut steps = std::collections::VecDeque::new();
        let Some(custom) = self.observed.ui.header.custom.as_ref() else {
            return Ok(steps);
        };
        let (component, dispose) = (custom.component, custom.dispose);
        if let Some(handler) = custom.set_expanded {
            self.observed
                .register(format!("expansion {handler}"), handler);
        }
        self.observed
            .register(format!("header {component}"), component);
        steps.extend(self.dispose_step(dispose)?);
        Ok(steps)
    }

    /// Retains every optional capability of a factory component in its shown slot.
    fn factory_slot(
        &self,
        key: String,
        built: host::FactoryComponent,
    ) -> wasmtime::Result<WidgetSlot> {
        let dispose = built
            .dispose
            .map(|dispose| self.table.get(&dispose).map(|identity| identity.0))
            .transpose()?;
        Ok(WidgetSlot {
            key,
            component: built.component.rep(),
            dispose,
            set_expanded: built.set_expanded.as_ref().map(Resource::rep),
            width: built.width,
        })
    }

    /// Shows the editor a factory built, keeping the custom editor it extends.
    fn show_built_editor(
        &mut self,
        factory: u32,
        built: host::EditorBuilt,
    ) -> wasmtime::Result<()> {
        let custom = built
            .custom
            .map(|custom| self.table.delete(custom).map(|widget| widget.0))
            .transpose()?;
        let slot = EditorSlot {
            factory,
            component: built.component.rep(),
            editor: built.editor.rep(),
            custom,
        };
        show_editor(&mut self.observed, slot);
        Ok(())
    }

    /// Shows a built widget, or returns the failure that ends its operation.
    pub(super) fn settle_nested(
        &mut self,
        nested: Nested,
        value: host::LocalResult,
    ) -> wasmtime::Result<Option<String>> {
        match (nested, value) {
            (Nested::CredentialBackend(expected), host::LocalResult::Unit(actual)) => {
                assert_eq!(actual.err(), expected);
                Ok(None)
            }
            (Nested::CredentialFallback(expected), host::LocalResult::AuthFallback(actual)) => {
                assert_eq!(actual, expected);
                Ok(actual.err())
            }
            (Nested::Footer, host::LocalResult::Factory(Ok(built))) => {
                let slot = self.factory_slot("footer".to_owned(), built)?;
                self.observed.ui.footer.custom = Some(slot);
                Ok(None)
            }
            (Nested::Header, host::LocalResult::Factory(Ok(built))) => {
                let slot = self.factory_slot("header".to_owned(), built)?;
                self.observed.ui.header.install(Some(slot));
                Ok(None)
            }
            (Nested::Factory(key), host::LocalResult::Factory(Ok(built))) => {
                let slot = self.factory_slot(key, built)?;
                self.observed.ui.widgets.push(slot);
                Ok(None)
            }
            (Nested::Editor(factory), host::LocalResult::EditorFactory(Ok(built))) => {
                self.show_built_editor(factory, built)?;
                Ok(None)
            }
            (Nested::Provider, host::LocalResult::AutocompleteFactory(Ok(provider))) => {
                self.stack_provider(&provider);
                Ok(None)
            }
            (Nested::Editor(_), host::LocalResult::EditorFactory(Err(error)))
            | (Nested::Provider, host::LocalResult::AutocompleteFactory(Err(error)))
            | (
                _,
                host::LocalResult::Factory(Err(error)) | host::LocalResult::Synchronous(Err(error)),
            )
            | (Nested::Invalidate, host::LocalResult::Unit(Err(error)))
            | (Nested::Widget(..) | Nested::Shortcut(..), host::LocalResult::Widget(Err(error))) => {
                Ok(Some(error))
            }
            (Nested::Widget(key, Some(index)), host::LocalResult::Widget(Ok(answer))) => {
                if let host::WidgetAnswer::Component(component) = answer {
                    let id = self.table.get(&component)?.0;
                    let list = super::widget_lists::list(&mut self.observed, key);
                    list.map_err(wasmtime::Error::msg)?.submenu = Some((index, id));
                }
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    /// Records one theme request made through `this` and answers it from the host's state.
    fn ui_themed(
        &mut self,
        this: &Resource<Ui>,
        method: &str,
        operand: &Value,
    ) -> wasmtime::Result<Result<Value, String>> {
        let key = self.table.get(this)?.0;
        Ok(themed(&mut self.observed, key, method, operand))
    }

    /// Lends the recorded palette `key` as a theme resource.
    fn lend_theme(&mut self, key: &Value) -> wasmtime::Result<Resource<Theme>> {
        self.push(Theme(key.as_str().unwrap_or_default().to_owned()))
    }

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
        let operands = widget(&key, content.as_deref(), options);
        let ukey = self.table.get(&this)?.0;
        if let Err(error) = call(&mut self.observed, ukey, "setWidget", &operands) {
            return self.routed(Err(error));
        }
        let steps = self.take_widgets(&key)?;
        self.push(super::host::Route {
            steps,
            running: None,
            rows: None,
            result: Some(Ok(host::RouteValue::Unit)),
        })
    }

    fn set_footer(
        &mut self,
        this: Resource<Ui>,
        factory: Option<Resource<super::host::Identity>>,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        let ukey = self.table.get(&this)?.0;
        let operands = json!([factory.is_some()]);
        if let Err(error) = call(&mut self.observed, ukey, "setFooter", &operands) {
            return self.routed(Err(error));
        }
        let mut steps = std::collections::VecDeque::new();
        if let Some(old) = self.observed.ui.footer.custom.take() {
            if let Some(handler) = old.set_expanded {
                self.observed
                    .register(format!("expansion {handler}"), handler);
            }
            self.observed
                .register(format!("footer {}", old.component), old.component);
            steps.extend(self.dispose_step(old.dispose)?);
        }
        if let Some(factory) = factory {
            let factory = self.local_factory(&factory, true)?;
            steps.push_back((host::LocalCall::Factory(factory), Nested::Footer));
        }
        self.push(super::host::Route {
            steps,
            running: None,
            rows: None,
            result: Some(Ok(host::RouteValue::Unit)),
        })
    }

    fn set_header(
        &mut self,
        this: Resource<Ui>,
        factory: Option<Resource<super::host::Identity>>,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        let ukey = self.table.get(&this)?.0;
        let operands = json!([factory.is_some()]);
        if let Err(error) = call(&mut self.observed, ukey, "setHeader", &operands) {
            return self.routed(Err(error));
        }
        if !self.observed.ui.header.ready {
            return self.routed(Ok(()));
        }
        let mut steps = self.take_header()?;
        match factory {
            None => self.observed.ui.header.install(None),
            Some(factory) => {
                let factory = self.local_factory(&factory, false)?;
                steps.push_back((host::LocalCall::Factory(factory), Nested::Header));
            }
        }
        self.push(super::host::Route {
            steps,
            running: None,
            rows: None,
            result: Some(Ok(host::RouteValue::Unit)),
        })
    }

    fn set_widget_factory(
        &mut self,
        this: Resource<Ui>,
        key: String,
        factory: Resource<super::host::Identity>,
        options: Option<host::ExtensionWidgetOptions>,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        let placement = options
            .and_then(|options| options.placement)
            .map_or_else(|| "AboveEditor".to_owned(), variant);
        let ukey = self.table.get(&this)?.0;
        let operands = json!([key, placement]);
        if let Err(error) = call(&mut self.observed, ukey, "setWidgetFactory", &operands) {
            return self.routed(Err(error));
        }
        let mut steps = self.take_widgets(&key)?;
        let factory = self.local_factory(&factory, false)?;
        steps.push_back((host::LocalCall::Factory(factory), Nested::Factory(key)));
        self.push(super::host::Route {
            steps,
            running: None,
            rows: None,
            result: Some(Ok(host::RouteValue::Unit)),
        })
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

    fn theme(&mut self, this: Resource<Ui>) -> wasmtime::Result<Result<Resource<Theme>, String>> {
        match self.ui_themed(&this, "theme", &Value::Null)? {
            Ok(key) => Ok(Ok(self.lend_theme(&key)?)),
            Err(error) => Ok(Err(error)),
        }
    }

    fn get_all_themes(
        &mut self,
        this: Resource<Ui>,
    ) -> wasmtime::Result<Result<Vec<host::ThemeInfo>, String>> {
        let themes = self.ui_themed(&this, "getAllThemes", &Value::Null)?;
        Ok(themes.map(|_| {
            THEMES
                .iter()
                .map(|(name, path)| host::ThemeInfo {
                    name: (*name).to_owned(),
                    path: path.map(str::to_owned),
                })
                .collect()
        }))
    }

    fn get_theme(
        &mut self,
        this: Resource<Ui>,
        name: String,
    ) -> wasmtime::Result<Result<Option<Resource<Theme>>, String>> {
        let loaded = self.ui_themed(&this, "getTheme", &json!(name))?;
        match loaded {
            Ok(key @ Value::String(_)) => Ok(Ok(Some(self.lend_theme(&key)?))),
            Ok(_) => Ok(Ok(None)),
            Err(error) => Ok(Err(error)),
        }
    }

    fn set_theme_name(
        &mut self,
        this: Resource<Ui>,
        name: String,
    ) -> wasmtime::Result<Result<host::ThemeSetResult, String>> {
        let result = self.ui_themed(&this, "setThemeName", &json!(name))?;
        Ok(result.map(|result| set_result(&result)))
    }

    fn set_theme(
        &mut self,
        this: Resource<Ui>,
        theme: Resource<Theme>,
    ) -> wasmtime::Result<Result<host::ThemeSetResult, String>> {
        let key = self.table.get(&theme)?.0.clone();
        let result = self.ui_themed(&this, "setTheme", &json!(key))?;
        Ok(result.map(|result| set_result(&result)))
    }

    fn get_tools_expanded(&mut self, this: Resource<Ui>) -> wasmtime::Result<Result<bool, String>> {
        let expanded = self.ui_themed(&this, "getToolsExpanded", &Value::Null)?;
        Ok(expanded.map(|expanded| expanded == true))
    }

    fn set_tools_expanded(
        &mut self,
        this: Resource<Ui>,
        expanded: bool,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        let result = self.ui_themed(&this, "setToolsExpanded", &json!(expanded))?;
        self.routed(result.map(drop))
    }

    fn get_editor_text(&mut self, this: Resource<Ui>) -> wasmtime::Result<Result<String, String>> {
        let key = self.table.get(&this)?.0;
        Ok(get_editor_text(&mut self.observed, key))
    }

    fn set_editor_component(
        &mut self,
        this: Resource<Ui>,
        factory: Option<Resource<super::host::Identity>>,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        let key = self.table.get(&this)?.0;
        let target = factory
            .as_ref()
            .map(|factory| self.table.get(factory).map(|identity| identity.0))
            .transpose()?;
        let old = match set_editor_component(&mut self.observed, key, target) {
            Ok(old) => old,
            Err(error) => return self.routed(Err(error)),
        };
        if let Some(old) = old {
            self.observed
                .register(format!("editor component {}", old.component), old.component);
            self.observed
                .register(format!("editor {}", old.editor), old.editor);
        }
        let mut steps = std::collections::VecDeque::new();
        if let Some(target) = target {
            let call = host::LocalEditorFactory {
                target,
                tui: self.push(Tui)?,
                theme: self.lend_editor_theme()?,
                keybindings: self.push(super::widget_component::KeybindingsResource(0))?,
                emitter: self.table.push(super::bus_host::Emitter { key: 0 })?,
            };
            steps.push_back((host::LocalCall::EditorFactory(call), Nested::Editor(target)));
        }
        self.push(super::host::Route {
            steps,
            running: None,
            rows: None,
            result: Some(Ok(host::RouteValue::Unit)),
        })
    }

    fn add_autocomplete_provider(
        &mut self,
        this: Resource<Ui>,
        factory: Resource<super::host::Identity>,
    ) -> wasmtime::Result<Resource<super::host::Route>> {
        self.add_provider(&this, &factory)
    }

    fn get_editor_component(
        &mut self,
        this: Resource<Ui>,
    ) -> wasmtime::Result<Result<Option<host::CurrentEditorFactory>, String>> {
        let key = self.table.get(&this)?.0;
        let current = match get_editor_component(&mut self.observed, key) {
            Ok(current) => current,
            Err(error) => return Ok(Err(error)),
        };
        Ok(Ok(match current {
            None => None,
            Some(Ok(id)) => Some(host::CurrentEditorFactory::Authored(id)),
            Some(Err(custom)) => Some(host::CurrentEditorFactory::Host(
                self.push(super::editor_component::ForeignFactory(custom))?,
            )),
        }))
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
    async fn custom(
        accessor: &Accessor<State, Self>,
        this: Resource<Ui>,
        factory: Resource<super::host::Identity>,
        options: Option<host::CustomUiOptions>,
    ) -> wasmtime::Result<Result<(), String>> {
        super::custom_host::run(accessor, this, factory, options).await
    }
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

impl host::HostTui for State {
    fn full_redraws(&mut self, _: Resource<Tui>) -> wasmtime::Result<Result<u64, String>> {
        self.observed.log("tui fullRedraws");
        Ok(Ok(17))
    }
    fn writer(
        &mut self,
        _: Resource<Tui>,
    ) -> wasmtime::Result<Result<Resource<super::widget_component::WidgetResource>, String>> {
        // shortcut: redraw requests only; use a real frame writer for other requests.
        Ok(Ok(self.push(super::widget_component::WidgetResource(0))?))
    }
    fn drop(&mut self, this: Resource<Tui>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

impl host::HostFooterData for State {
    fn get_git_branch(
        &mut self,
        _: Resource<FooterResource>,
    ) -> wasmtime::Result<Result<Option<String>, String>> {
        let branch = self.observed.ui.footer.read("getGitBranch");
        Ok(Ok(branch.as_str().map(str::to_owned)))
    }
    fn get_extension_statuses(
        &mut self,
        _: Resource<FooterResource>,
    ) -> wasmtime::Result<Result<Vec<(String, String)>, String>> {
        let statuses = self.observed.ui.footer.read("getExtensionStatuses");
        Ok(Ok(serde_json::from_value(statuses)?))
    }
    fn get_available_provider_count(
        &mut self,
        _: Resource<FooterResource>,
    ) -> wasmtime::Result<Result<f64, String>> {
        let count = self.observed.ui.footer.read("getAvailableProviderCount");
        Ok(Ok(count.as_f64().unwrap_or_default()))
    }
    fn on_branch_change(
        &mut self,
        _: Resource<FooterResource>,
        listener: Resource<super::host::Identity>,
    ) -> wasmtime::Result<Result<Resource<super::bus_host::Subscription>, String>> {
        match self.observed.bus.on("branch", listener.rep()) {
            Ok(subscription) => Ok(Ok(self.table.push(subscription)?)),
            Err(error) => Ok(Err(error)),
        }
    }
    fn drop(&mut self, this: Resource<FooterResource>) -> wasmtime::Result<()> {
        self.release(this)
    }
}

impl host::HostToolInvalidator for State {
    fn invalidate(&mut self, this: Resource<Invalidator>) -> wasmtime::Result<Result<(), String>> {
        let row = self.table.get(&this)?.0.clone();
        self.observed.log(format!("invalidate {row}"));
        Ok(Ok(()))
    }
    fn drop(&mut self, this: Resource<Invalidator>) -> wasmtime::Result<()> {
        self.release(this)
    }
}
