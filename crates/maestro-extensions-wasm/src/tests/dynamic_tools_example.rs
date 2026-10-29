//! The shipped dynamic-tools factory exercised through the controlled and component hosts.
use serde::Deserialize;
use serde_json::{Value, json};
use wasmtime::Engine;
use wasmtime::component::Component;

use super::controlled::{Controlled, Flag};
use super::example_support::{
    Command, assert_registered_command, invoke_command_on_component, invoke_tool_on_component,
    registered_metadata, release_all, result_value, start,
};
use super::host::Harness;
use super::host_family::guest::Capabilities as ComponentCapabilities;
use super::host_family::guest::ToolCapabilities as ComponentToolCapabilities;
use super::observed::Observed;
use super::ui_host;
use crate::bindings::exports::maestro::extension::guest::Decision as GuestDecision;
use crate::bindings::exports::maestro::extension::guest::ToolInvocation;
use crate::component_adapter::{Capabilities, Exports, ToolCapabilities};
use crate::{Extension, ExtensionAPI, ExtensionFuture};

#[path = "../../../../examples/extensions/dynamic_tools.rs"]
mod dynamic_tools;

/// The same factory exported by the shipped component.
struct DynamicTools;

impl Extension for DynamicTools {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        dynamic_tools::dynamic_tools_extension(api)
    }
}

/// The message every supplied host rejection carries, with its embedded newline.
const FAILURE: &str = "host refused Ω\ncontrolled";

/// The registrations every factory instance makes, in the order the host received them.
const REGISTERED: [&str; 2] = ["event session_start", "command add-echo-tool"];

/// The session-start document every recorded event step delivers.
fn session_start() -> Value {
    json!({ "type": "session_start", "reason": "new" })
}

/// The recorded rows these tests consume.
fn corpus() -> Corpus {
    serde_json::from_str(include_str!("dynamic_tools_expected.json")).expect("the recorded corpus")
}

/// The recorded row named `name`.
fn row(name: &str) -> Case {
    corpus()
        .rows
        .into_iter()
        .find(|case| case.name == name)
        .expect("a recorded row")
}

/// Replays the recorded rows `keep` selects through the instances `open` starts.
fn replay<H: Host>(
    keep: impl Fn(&Case) -> bool,
    mut open: impl FnMut(&[Registration]) -> Result<H, String>,
) -> Result<(), String> {
    let corpus = corpus();
    let rows: Vec<&Case> = corpus.rows.iter().filter(|case| keep(case)).collect();
    assert!(!rows.is_empty(), "recorded rows");
    for case in rows {
        let mut hosts = Vec::new();
        for recorded in &case.registrations {
            hosts.push(open(recorded)?);
        }
        checked(case, &mut hosts);
    }
    Ok(())
}

/// Runs every recorded step against the instance it selects, then releases every instance.
fn checked<H: Host>(case: &Case, hosts: &mut [H]) {
    assert_eq!(hosts.len(), case.instances, "{}: instances", case.name);
    assert_eq!(
        case.observations.len(),
        case.steps.len(),
        "{}: steps",
        case.name
    );
    for (index, step) in case.steps.iter().enumerate() {
        let answer = hosts[step.instance.unwrap_or(0)].run(step);
        let expected = &case.observations[index];
        let at = format!("{} step {index}", case.name);
        assert_eq!(answer.effects, expected.effects, "{at}: effects");
        assert_eq!(answer.visible, expected.visible, "{at}: visible");
        assert_eq!(answer.result, expected.result, "{at}: result");
        assert_eq!(answer.error, expected.error, "{at}: error");
        assert_eq!(answer.updates, 0, "{at}: progress");
    }
    for host in hosts {
        host.release();
    }
}

/// Asserts the host received exactly the recorded registrations, in that order.
fn assert_registrations(observed: &Observed, recorded: &[Registration]) {
    let lines: Vec<String> = recorded
        .iter()
        .map(|registration| match registration {
            Registration::Event { event } => format!("register event {event}"),
            Registration::Command { command } => format!("register command {}", command.name),
        })
        .collect();
    assert_eq!(observed.transcript, lines, "the registrations in order");
    for registration in recorded {
        if let Registration::Command { command } = registration {
            assert_registered_command(observed, command);
        }
    }
    assert!(
        observed.tools.is_empty(),
        "initialization registers no tool"
    );
}

/// The tool names the instance admitted, in admission order.
fn visible(observed: &Observed) -> Vec<String> {
    observed.tools.iter().map(|text| tool_name(text)).collect()
}

/// The name one registration metadata document carries.
fn tool_name(text: &str) -> String {
    let value: Value = serde_json::from_str(text).expect("tool metadata");
    value["name"].as_str().expect("a tool name").to_owned()
}

/// Every name a host retained: the initialization registrations, then each admitted tool.
fn retained_names(observed: &Observed) -> Vec<String> {
    let mut names: Vec<String> = REGISTERED.iter().map(|name| (*name).to_owned()).collect();
    names.extend(
        visible(observed)
            .into_iter()
            .map(|name| format!("tool {name}")),
    );
    names
}

/// The metadata document a tool registration sent, without its declared kinds.
fn metadata(text: &str) -> Value {
    registered_metadata(text)
}

/// The tool registration attempts and notifications the host recorded since `mark`.
fn effects(observed: &Observed, mark: Mark) -> Vec<Value> {
    let mut attempts = observed
        .attempts
        .iter()
        .enumerate()
        .skip(mark.attempts)
        .peekable();
    let mut effects = Vec::new();
    for (index, line) in observed.transcript.iter().enumerate().skip(mark.lines) {
        while let Some((_, attempt)) = attempts.next_if(|(_, attempt)| attempt.position <= index) {
            effects.push(json!({"registerTool": metadata(&attempt.metadata)}));
        }
        if let Some(receipt) = ui_host::receipt(line).filter(|receipt| receipt.method == "notify") {
            effects.push(json!({"notify": {
                "message": receipt.operands[0],
                "kind": receipt.operands[1].as_str().map(str::to_lowercase),
            }}));
        }
    }
    for (_, attempt) in attempts {
        effects.push(json!({"registerTool": metadata(&attempt.metadata)}));
    }
    effects
}

/// Supplies the decisions one recorded step asks its host for.
fn prepare(observed: &mut Observed, step: &Step) {
    let refusing = |rejection| (step.reject == Some(rejection)).then(|| FAILURE.to_owned());
    observed.registrations.error = refusing(Rejection::RegisterTool);
    if step.reject == Some(Rejection::Notify) {
        observed.ui.reject("notify", FAILURE);
    }
    observed.catalog.control(
        &json!({"state": {
            "hasUI": step.has_ui.unwrap_or(true),
            "error": refusing(Rejection::Ui),
        }})
        .to_string(),
    );
}

/// Clears the decisions a recorded step supplied.
fn disarm(observed: &mut Observed) {
    observed.registrations.error = None;
    observed.catalog.control(r#"{"state":{}}"#);
}

/// Where one recorded step starts: the observations made before it ran.
#[derive(Clone, Copy, Default)]
struct Mark {
    /// Transcript lines recorded before the step.
    lines: usize,
    /// Tool registration attempts made before the step.
    attempts: usize,
}

/// What one recorded step produced.
struct Answer {
    /// Tool registration attempts and notifications, in the order the host recorded them.
    effects: Vec<Value>,
    /// The tool names the instance has admitted, in admission order.
    visible: Vec<String>,
    /// Partial results the tool execution delivered.
    updates: usize,
    /// The answer a tool execution returned.
    result: Option<Value>,
    /// The error the operation ended with.
    error: Option<String>,
}

/// One factory instance a recorded row runs against.
trait Host {
    /// Runs `act` against what this instance's host observed.
    fn with_observed(&mut self, act: impl FnOnce(&mut Observed));

    /// Delivers the recorded session start event.
    fn session_start(&mut self) -> Result<(), String>;

    /// Delivers one recorded command invocation.
    fn command(&mut self, args: &str) -> Result<(), String>;

    /// Delivers one recorded tool execution.
    fn execute(&mut self, step: &Step) -> Result<Value, String>;

    /// Releases every registration this instance made.
    fn release(&mut self);

    /// Where one recorded step starts.
    fn mark(&mut self) -> Mark {
        let mut mark = Mark::default();
        self.with_observed(|observed| {
            mark = Mark {
                lines: observed.transcript.len(),
                attempts: observed.attempts.len(),
            };
        });
        mark
    }

    /// Runs one recorded step and reads what it produced.
    fn run(&mut self, step: &Step) -> Answer {
        self.with_observed(|observed| prepare(observed, step));
        let mark = self.mark();
        let (result, error) = match step.op {
            Op::SessionStart => (None, self.session_start().err()),
            Op::Command => (
                None,
                self.command(step.args.as_deref().unwrap_or_default()).err(),
            ),
            Op::Execute => match self.execute(step) {
                Ok(value) => (Some(value), None),
                Err(error) => (None, Some(error)),
            },
        };
        self.with_observed(disarm);
        let mut answer = Answer {
            effects: Vec::new(),
            visible: Vec::new(),
            updates: 0,
            result,
            error,
        };
        self.with_observed(|observed| {
            answer.effects = effects(observed, mark);
            answer.visible = visible(observed);
            answer.updates = observed.updates.len();
        });
        answer
    }
}

/// The controlled adapter with the keys the shipped factory registered.
struct ControlledHost {
    /// The host that recorded what the factory asked for.
    host: Controlled,
    /// The adapter serving the factory's callbacks.
    exports: Exports<Controlled>,
    /// The registered session-start handler.
    event: u32,
    /// The registered command.
    command: u32,
}

impl ControlledHost {
    /// Starts the shipped factory on a fresh controlled host.
    fn new(recorded: &[Registration]) -> Result<Self, String> {
        let (host, exports) = super::block_on(start::<DynamicTools>())?;
        assert_registrations(&host.observed(), recorded);
        let event = host.observed().callback("event session_start")?;
        let command = host.observed().callback("command add-echo-tool")?;
        Ok(Self {
            host,
            exports,
            event,
            command,
        })
    }
}

impl Host for ControlledHost {
    fn with_observed(&mut self, act: impl FnOnce(&mut Observed)) {
        act(&mut self.host.observe());
    }

    fn session_start(&mut self) -> Result<(), String> {
        let event = session_start();
        let outcome = super::block_on(self.exports.invoke_event(
            self.event,
            event.to_string(),
            Capabilities {
                ctx: self.host.context("/work"),
                signal: None,
            },
        ))?;
        match outcome.decision {
            GuestDecision::Returned(_) => Ok(()),
            GuestDecision::Failed(message) => Err(message),
        }
    }

    fn command(&mut self, args: &str) -> Result<(), String> {
        super::block_on(self.exports.invoke_command(
            self.command,
            args.to_owned(),
            self.host.command_context("/work"),
        ))
    }

    fn execute(&mut self, step: &Step) -> Result<Value, String> {
        let tool = step.tool.as_deref().ok_or("a recorded tool")?;
        let key = self.host.observed().callback(&format!("tool {tool}"))?;
        let params = step.params.as_ref().ok_or("recorded params")?;
        let flag = Flag::default();
        if step.aborted {
            flag.abort();
        }
        let encoded = super::block_on(self.exports.invoke_tool(
            key,
            ToolInvocation {
                tool_call_id: "controlled-call".into(),
                params: params.to_string(),
            },
            ToolCapabilities {
                ctx: self.host.context("/work"),
                signal: Some(self.host.signal(&flag)),
                update: step.progress.then(|| self.host.update()),
            },
        ))?;
        Ok(result_value(&encoded))
    }

    fn release(&mut self) {
        let names = retained_names(&self.host.observed());
        release_all(&self.host, &names);
    }
}

/// The component host and the keys the built factory registered.
struct ComponentHost {
    /// The instantiated component.
    harness: Harness,
    /// The registered session-start handler.
    event: u32,
    /// The registered command.
    command: u32,
}

impl ComponentHost {
    /// Instantiates the built example and looks up its registration.
    fn new(
        engine: &Engine,
        component: &Component,
        recorded: &[Registration],
    ) -> Result<Self, String> {
        let mut harness = super::block_on(Harness::from_compiled(engine, component))
            .map_err(|error| error.to_string())?;
        assert_registrations(harness.observed(), recorded);
        Ok(Self {
            event: harness
                .callback("event session_start")
                .map_err(|error| error.to_string())?,
            command: harness
                .callback("command add-echo-tool")
                .map_err(|error| error.to_string())?,
            harness,
        })
    }
}

impl Host for ComponentHost {
    fn with_observed(&mut self, act: impl FnOnce(&mut Observed)) {
        act(self.harness.observed());
    }

    fn session_start(&mut self) -> Result<(), String> {
        let key = self.event;
        let capabilities = ComponentCapabilities {
            ctx: self
                .harness
                .ordinary("/work")
                .map_err(|error| error.to_string())?,
            signal: None,
        };
        let exports = self.harness.exports.clone();
        let document = session_start().to_string();
        let run = super::block_on(self.harness.store.run_concurrent(async |accessor| {
            exports
                .call_invoke_event(accessor, super::host::borrow(key), document, capabilities)
                .await
        }))
        .map_err(|error| error.to_string())?;
        let outcome = run.map_err(|error| error.to_string())??;
        match super::documents::Answer::of(super::host_family::delivery(outcome))?.ended {
            super::documents::Ended::Returned(_) => Ok(()),
            super::documents::Ended::Failed(message) => Err(message),
        }
    }

    fn command(&mut self, args: &str) -> Result<(), String> {
        invoke_command_on_component(&mut self.harness, self.command, args, false)
    }

    fn execute(&mut self, step: &Step) -> Result<Value, String> {
        let tool = step.tool.as_deref().ok_or("a recorded tool")?;
        let key = self
            .harness
            .callback(&format!("tool {tool}"))
            .map_err(|error| error.to_string())?;
        let params = step.params.as_ref().ok_or("recorded params")?;
        let resources = ComponentToolCapabilities {
            ctx: self
                .harness
                .ordinary("/work")
                .map_err(|error| error.to_string())?,
            signal: Some(wasmtime::component::Resource::new_own(
                self.harness
                    .flag(step.aborted)
                    .map_err(|error| error.to_string())?,
            )),
            update: step
                .progress
                .then(|| self.harness.update().map_err(|error| error.to_string()))
                .transpose()?,
        };
        let encoded = invoke_tool_on_component(&mut self.harness, key, params, resources)?;
        Ok(result_value(&encoded))
    }

    fn release(&mut self) {
        for name in retained_names(self.harness.observed()) {
            let key = self.harness.callback(&name).expect("a registration");
            super::block_on(self.harness.release(key)).expect("a released registration");
        }
        assert_eq!(self.harness.observed().live, 0, "the host lends nothing");
    }
}

/// Startup registers one session hook and one command, in that order, and no tool yet.
#[test]
fn dynamic_tools_registers_deferred_tools_and_command() -> Result<(), String> {
    let case = row("accepted-\"echo\"");
    let mut host = ControlledHost::new(&case.registrations[0])?;
    host.with_observed(|observed| {
        assert!(visible(observed).is_empty(), "no tool is registered yet");
    });
    host.release();
    Ok(())
}

/// Every recorded name sequence registers its normalized name or warns, and registers nothing else.
#[test]
fn dynamic_tools_names_follow_argument_rules() -> Result<(), String> {
    replay(|case| case.group == Group::Names, ControlledHost::new)
}

/// Repeated starts and a command after initialization expose and execute the tools they add.
#[test]
fn dynamic_tools_session_and_command_register_live_tools() -> Result<(), String> {
    replay(|case| case.group == Group::Lifecycle, ControlledHost::new)
}

/// Three spellings of one name register it once and keep the first callback.
#[test]
fn dynamic_tools_duplicate_names_keep_first_registration() -> Result<(), String> {
    replay(
        |case| case.name == "normalized-duplicates",
        ControlledHost::new,
    )
}

/// The message rows echo through both tool families, each behind its own prefix; the
/// extra-argument and unused-cancellation rows invoke only the session tool.
#[test]
fn dynamic_tools_echo_preserves_messages() -> Result<(), String> {
    replay(|case| case.group == Group::Echo, ControlledHost::new)
}

/// A rejected host request keeps the effects that came before it and the next call retries.
#[test]
fn dynamic_tools_host_failures_keep_prior_state() -> Result<(), String> {
    replay(|case| case.group == Group::Errors, ControlledHost::new)
}

/// The built single copy replays every recorded row through the callbacks it registered.
#[test]
fn dynamic_tools_component_matches_registered_callbacks() -> Result<(), String> {
    let path = super::build::example_component("maestro-extensions-wasm", "dynamic_tools")?;
    let (engine, component) = super::build::compile(&path).map_err(|error| error.to_string())?;
    replay(
        |_| true,
        |recorded| ComponentHost::new(&engine, &component, recorded),
    )
}

/// The recorded registrations and rows.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// The recorded rows.
    rows: Vec<Case>,
}

/// One recorded row: its inputs and the observations every step must produce.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// The row name in the recorded corpus.
    name: String,
    /// The behavior group the row belongs to.
    group: Group,
    /// The operations the row performs, in order.
    steps: Vec<Step>,
    /// How many factory instances the row uses.
    instances: usize,
    /// The registrations each instance made, in registration order.
    registrations: Vec<Vec<Registration>>,
    /// The observations each step must produce, in step order.
    observations: Vec<Observation>,
}

/// The behavior group a recorded row belongs to.
#[derive(Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Group {
    /// Name normalization and admission.
    Names,
    /// Session start, repeated start and command ordering.
    Lifecycle,
    /// Duplicate admission and independent factory state.
    State,
    /// Echoed messages and invocation handles.
    Echo,
    /// Rejected host requests.
    Errors,
}

/// One recorded operation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Step {
    /// The operation kind.
    op: Op,
    /// The command arguments.
    #[serde(default)]
    args: Option<String>,
    /// The tool whose execution the step invokes.
    #[serde(default)]
    tool: Option<String>,
    /// The arguments the tool invocation supplies.
    #[serde(default)]
    params: Option<Value>,
    /// The factory instance the step runs against, when it is not the first.
    #[serde(default)]
    instance: Option<usize>,
    /// The host request the step rejects.
    #[serde(default)]
    reject: Option<Rejection>,
    /// The UI availability the step's contexts report.
    #[serde(default, rename = "hasUI")]
    has_ui: Option<bool>,
    /// Whether the tool invocation carries an already-raised signal.
    #[serde(default)]
    aborted: bool,
    /// Whether the tool invocation carries a progress handle.
    #[serde(default)]
    progress: bool,
}

/// The operation kind one recorded step delivers.
#[derive(Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Op {
    /// A session start event.
    SessionStart,
    /// A command invocation.
    Command,
    /// A tool execution.
    Execute,
}

/// The host request one recorded step rejects.
#[derive(Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum Rejection {
    /// The next tool registration.
    RegisterTool,
    /// The next notification.
    Notify,
    /// The next UI acquisition.
    Ui,
}

/// One registration a factory instance made.
#[derive(Deserialize)]
#[serde(untagged)]
enum Registration {
    /// A session-start handler.
    Event { event: String },
    /// A command with its metadata.
    Command { command: Command },
}

/// The observations one recorded step must produce.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    /// Tool registration attempts and notifications, in the order the host recorded them.
    effects: Vec<Value>,
    /// The tool names the instance admits, in admission order.
    visible: Vec<String>,
    /// The answer a tool execution returned.
    result: Option<Value>,
    /// The error the operation ends with.
    error: Option<String>,
}
