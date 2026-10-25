//! The shipped reload-runtime factory exercised through both hosts.
use serde::Deserialize;
use serde_json::Value;

use super::controlled::Controlled;
use super::example_support::{
    Command, Observation, assert_registered_command, invoke_command_on_component,
    invoke_tool_on_component, registered_metadata, release_all, result_value, start,
};
use super::host::Harness;
use super::host_family::guest::ToolCapabilities as ComponentCapabilities;
use crate::component_adapter::{Exports, ToolCapabilities};
use crate::{Extension, ExtensionAPI, ExtensionFuture};

#[path = "../../../../examples/extensions/reload_runtime.rs"]
mod reload_runtime;

/// The same factory exported by the shipped component.
struct ReloadRuntime;

impl Extension for ReloadRuntime {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        reload_runtime::factory(api)
    }
}

/// What one invocation observed and the answer it returned.
struct Answer<T> {
    /// The observations the invocation made.
    lines: Vec<String>,
    /// The answer it returned, or the error it produced.
    outcome: Result<T, String>,
}

/// A host that runs the factory's registered command and tool.
trait Host {
    /// Runs the command with `args` and returns its own observations; `held` keeps its reload
    /// pending until the test saw it and released it.
    fn command(&mut self, args: &str, held: bool) -> Answer<()>;

    /// Runs the tool once with `params` and a possibly already aborted signal.
    fn tool(&mut self, params: &Value, aborted: bool) -> Answer<String>;

    /// Supplies the answer of the next `reload` request or of the next message delivery.
    fn reject(&mut self, answer: Supplied, error: Option<String>);

    /// The observations made since `mark`.
    fn since(&self, mark: usize) -> Vec<String>;

    /// The current observation mark.
    fn mark(&self) -> usize;

    /// How many progress results the host recorded.
    fn progress(&self) -> usize;
}

/// The supplied host answer a case selects.
#[derive(Clone, Copy)]
enum Supplied {
    /// The command's reload request.
    Reload,
    /// The tool's message delivery.
    Delivery,
}

/// The controlled adapter with the keys the shipped factory registered.
struct ControlledHost {
    /// The host that recorded what the factory asked for.
    host: Controlled,
    /// The adapter serving the factory's callbacks.
    exports: Exports<Controlled>,
    /// The registered command.
    command: u32,
    /// The registered tool.
    tool: u32,
}

impl Host for ControlledHost {
    fn command(&mut self, args: &str, held: bool) -> Answer<()> {
        let mark = self.mark();
        let host = &self.host;
        let exports = &self.exports;
        let key = self.command;
        let outcome = super::block_on(async move {
            let context = host.command_context("/work");
            let run = exports.invoke_command(key, args.to_owned(), context);
            if !held {
                return run.await;
            }
            let (pending, entered, open) = super::observed::gate();
            host.observe().hold_next_idle(pending);
            let release = async {
                entered.await.unwrap();
                host.observe().suspended();
                open.send(()).unwrap();
            };
            super::join::alongside(run, release).await
        });
        Answer {
            lines: self.since(mark),
            outcome,
        }
    }

    fn tool(&mut self, params: &Value, aborted: bool) -> Answer<String> {
        let mark = self.mark();
        let host = &self.host;
        let exports = &self.exports;
        let key = self.tool;
        let outcome = super::block_on(async move {
            let flag = super::controlled::Flag::default();
            if aborted {
                flag.abort();
            }
            exports
                .invoke_tool(
                    key,
                    crate::bindings::exports::maestro::extension::guest::ToolInvocation {
                        tool_call_id: "controlled-call".into(),
                        params: params.to_string(),
                    },
                    ToolCapabilities {
                        ctx: host.context("/work"),
                        signal: Some(host.signal(&flag)),
                        update: Some(host.update()),
                    },
                )
                .await
        });
        Answer {
            lines: self.since(mark),
            outcome,
        }
    }

    fn reject(&mut self, answer: Supplied, error: Option<String>) {
        match answer {
            Supplied::Reload => self.host.observe().replacement.error = error,
            Supplied::Delivery => self.host.observe().actions.error = error,
        }
    }

    fn since(&self, mark: usize) -> Vec<String> {
        self.host.observed().transcript[mark..].to_vec()
    }

    fn mark(&self) -> usize {
        self.host.observed().transcript.len()
    }

    fn progress(&self) -> usize {
        self.host.observed().updates.len()
    }
}

/// Starts the shipped factory on the controlled adapter.
fn controlled() -> Result<ControlledHost, String> {
    let (host, exports) = super::block_on(start::<ReloadRuntime>())?;
    let command = host.observed().callback("command reload-runtime")?;
    let tool = host.observed().callback("tool reload_runtime")?;
    Ok(ControlledHost {
        host,
        exports,
        command,
        tool,
    })
}

/// The component host and the keys the built factory registered.
struct ComponentHost {
    /// The instantiated component.
    harness: Harness,
    /// The registered command.
    command: u32,
    /// The registered tool.
    tool: u32,
}

impl ComponentHost {
    /// Instantiates the built example and looks up both registrations.
    fn new() -> Result<Self, String> {
        let harness = super::host::Harness::example("reload_runtime")?;
        let command = harness
            .callback("command reload-runtime")
            .map_err(|error| error.to_string())?;
        let tool = harness
            .callback("tool reload_runtime")
            .map_err(|error| error.to_string())?;
        Ok(Self {
            harness,
            command,
            tool,
        })
    }
}

impl Host for ComponentHost {
    fn command(&mut self, args: &str, held: bool) -> Answer<()> {
        let mark = self.mark();
        let outcome = invoke_command_on_component(&mut self.harness, self.command, args, held);
        Answer {
            lines: self.since(mark),
            outcome,
        }
    }

    fn tool(&mut self, params: &Value, aborted: bool) -> Answer<String> {
        let mark = self.mark();
        let resources = ComponentCapabilities {
            ctx: self.harness.ordinary("/work").expect("an ordinary context"),
            signal: Some(wasmtime::component::Resource::new_own(
                self.harness.flag(aborted).expect("a cancellation flag"),
            )),
            update: Some(self.harness.update().expect("a progress handle")),
        };
        let outcome = invoke_tool_on_component(&mut self.harness, self.tool, params, resources);
        Answer {
            lines: self.since(mark),
            outcome,
        }
    }

    fn reject(&mut self, answer: Supplied, error: Option<String>) {
        match answer {
            Supplied::Reload => self.harness.observed().replacement.error = error,
            Supplied::Delivery => self.harness.observed().actions.error = error,
        }
    }

    fn since(&self, mark: usize) -> Vec<String> {
        self.harness.store.data().observed.transcript[mark..].to_vec()
    }

    fn mark(&self) -> usize {
        self.harness.store.data().observed.transcript.len()
    }

    fn progress(&self) -> usize {
        self.harness.store.data().observed.updates.len()
    }
}

/// Exactly one command precedes exactly one tool, with the recorded metadata.
#[test]
fn reload_runtime_registers_command_and_tool() -> Result<(), String> {
    let started = controlled()?;
    let expected = corpus().metadata;
    assert_metadata(&started.host.observed(), &expected);
    release_all(&started.host, &recorded_order(&expected));
    Ok(())
}

/// Assert the recorded command and tool records against what the host received.
fn assert_metadata(observed: &super::observed::Observed, expected: &Metadata) {
    assert_registered_command(observed, &expected.command);
    assert_eq!(observed.tools.len(), 1);
    assert_eq!(tool_metadata(&observed.tools[0]), expected.tool);
}

/// The registered metadata without the transport's declared-kinds member.
fn tool_metadata(registered: &str) -> Tool {
    serde_json::from_value(registered_metadata(registered)).expect("the registered tool metadata")
}

/// The registration order the corpus records, in the host's own names.
fn recorded_order(metadata: &Metadata) -> Vec<String> {
    metadata
        .order
        .iter()
        .map(|registration| match registration.operation {
            Operation::RegisterCommand => format!("command {}", registration.name),
            Operation::RegisterTool => format!("tool {}", registration.name),
        })
        .collect()
}

/// The command awaits its reload request and returns nothing else.
#[test]
fn reload_runtime_command_awaits_reload_and_returns() -> Result<(), String> {
    let mut host = controlled()?;
    for case in selected(&corpus().commands, Outcome::Fulfilled) {
        command_case(&mut host, case);
    }
    release_all(&host.host, &recorded_order(&corpus().metadata));
    Ok(())
}

/// A refused reload request is returned unchanged, without a delivery or a fallback.
#[test]
fn reload_runtime_command_propagates_reload_failure() -> Result<(), String> {
    let mut host = controlled()?;
    for case in selected(&corpus().commands, Outcome::Rejected) {
        command_case(&mut host, case);
    }
    release_all(&host.host, &recorded_order(&corpus().metadata));
    Ok(())
}

/// The recorded cases of one outcome, which a case selection must find at least one of.
fn selected(cases: &[Case], outcome: Outcome) -> Vec<&Case> {
    let selected: Vec<&Case> = cases
        .iter()
        .filter(|case| case.expected.outcome == outcome)
        .collect();
    assert!(!selected.is_empty(), "recorded cases");
    selected
}

/// Runs one command case and asserts its recorded observations.
fn command_case<H: Host>(host: &mut H, case: &Case) {
    host.reject(Supplied::Reload, case.input.error.clone());
    let answer = host.command(&case.input.args, case.input.hold);
    assert_eq!(
        calls(&answer.lines),
        recorded_calls(&case.expected.calls),
        "{}: calls",
        case.name
    );
    assert_outcome(&answer.outcome, &case.expected, &case.name);
    assert!(
        !answer.lines.iter().any(|line| line.starts_with("action ")),
        "{}: the command delivered a message",
        case.name
    );
    assert_eq!(
        case.expected.pending_before_release,
        answer
            .lines
            .iter()
            .any(|line| line.starts_with("command suspended after replacement-call ")),
        "{}: the reload stayed pending",
        case.name
    );
}

/// The tool queues its command as a follow-up message before answering.
#[test]
fn reload_runtime_tool_queues_follow_up_before_result() -> Result<(), String> {
    let mut host = controlled()?;
    for case in selected(&corpus().tools, Outcome::Fulfilled) {
        tool_case(&mut host, case);
    }
    release_all(&host.host, &recorded_order(&corpus().metadata));
    Ok(())
}

/// A refused delivery is returned unchanged, without a queued result or a reload.
#[test]
fn reload_runtime_tool_propagates_delivery_failure() -> Result<(), String> {
    let mut host = controlled()?;
    for case in selected(&corpus().tools, Outcome::Rejected) {
        tool_case(&mut host, case);
    }
    release_all(&host.host, &recorded_order(&corpus().metadata));
    Ok(())
}

/// Runs one tool case and asserts its recorded calls and answers.
fn tool_case<H: Host>(host: &mut H, case: &Case) {
    host.reject(Supplied::Delivery, case.input.error.clone());
    let params = case.input.params.as_ref().expect("tool arguments");
    let progress = host.progress();
    let (mut lines, mut results) = (Vec::new(), Vec::new());
    for _ in 0..case.input.repetitions {
        let answer = host.tool(params, case.input.aborted);
        assert_outcome(&answer.outcome, &case.expected, &case.name);
        lines.extend(answer.lines);
        if let Ok(encoded) = answer.outcome {
            results.push(result_value(&encoded));
        }
    }
    // A rejected delivery reaches the message owner before any effect is admitted.
    let admitted: &[Call] = if case.expected.outcome == Outcome::Fulfilled {
        &case.expected.calls
    } else {
        &[]
    };
    assert_eq!(
        calls(&lines),
        recorded_calls(admitted),
        "{}: calls",
        case.name
    );
    assert_eq!(
        deliveries(&lines),
        recorded_deliveries(admitted),
        "{}: delivered messages",
        case.name
    );
    assert_eq!(results, case.expected.results, "{}: answers", case.name);
    assert_eq!(host.progress(), progress, "{}: progress", case.name);
}

/// The built single-copy example runs every recorded case through its callbacks.
#[test]
fn reload_runtime_component_uses_registered_callbacks() -> Result<(), String> {
    let mut started = ComponentHost::new()?;
    let corpus = corpus();
    assert_metadata(started.harness.observed(), &corpus.metadata);
    for case in &corpus.commands {
        command_case(&mut started, case);
    }
    for case in &corpus.tools {
        tool_case(&mut started, case);
    }
    for key in [started.command, started.tool] {
        super::block_on(started.harness.release(key)).map_err(|error| error.to_string())?;
    }
    assert_eq!(started.harness.observed().live, 0);
    Ok(())
}

/// The operations the host admitted, in order.
fn calls(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .filter_map(|line| {
            if let Some(rest) = line.strip_prefix("replacement-call ") {
                let call: Value = serde_json::from_str(rest).expect("reload observation");
                Some(call["group"].as_str().expect("call group").to_owned())
            } else {
                line.strip_prefix("action ")
                    .map(|rest| rest.split(' ').next().expect("action name").to_owned())
            }
        })
        .collect()
}

/// The messages the host admitted, with the content and options each carried.
fn deliveries(lines: &[String]) -> Vec<(Option<Value>, Option<Value>)> {
    super::example_support::observations(lines)
        .into_iter()
        .filter_map(|observation| match observation {
            Observation::Delivery { content, options } => Some((content, options)),
            Observation::Notify { .. } => None,
        })
        .collect()
}

/// Assert the outcome of one invocation against the recorded one.
fn assert_outcome<T>(outcome: &Result<T, String>, expected: &Expected, case: &str) {
    match expected.outcome {
        Outcome::Fulfilled => assert!(outcome.is_ok(), "{case}: {:?}", outcome.as_ref().err()),
        Outcome::Rejected => assert_eq!(outcome.as_ref().err(), expected.error.as_ref(), "{case}"),
    }
}

/// The operations the corpus records for one case.
fn recorded_calls(calls: &[Call]) -> Vec<String> {
    calls.iter().map(|call| call.operation.clone()).collect()
}

/// The messages the corpus records, in order.
fn recorded_deliveries(calls: &[Call]) -> Vec<(Option<Value>, Option<Value>)> {
    calls
        .iter()
        .filter(|call| call.operation == "sendUserMessage")
        .map(|call| (call.content.clone(), call.options.clone()))
        .collect()
}

/// The recorded observations these tests consume.
fn corpus() -> Corpus {
    serde_json::from_str(include_str!("reload_runtime_expected.json")).expect("recorded corpus")
}

/// The recorded registration and case observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// What the factory registers.
    metadata: Metadata,
    /// The command cases.
    commands: Vec<Case>,
    /// The tool cases.
    tools: Vec<Case>,
}

/// The command and tool the factory registers.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Metadata {
    /// The registrations in the order the host received them.
    order: Vec<Registration>,
    /// The registered command.
    command: Command,
    /// The registered tool.
    tool: Tool,
}

/// A registration operation of the recorded corpus.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum Operation {
    /// A command registration.
    RegisterCommand,
    /// A tool registration.
    RegisterTool,
}

/// One registration of the recorded corpus.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Registration {
    /// The registration operation the factory called.
    operation: Operation,
    /// The name it registered.
    name: String,
}

/// The registered tool's metadata.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Tool {
    /// The tool name.
    name: String,
    /// The tool label.
    label: String,
    /// The tool description.
    description: String,
    /// The accepted parameter schema.
    parameters: Value,
}

/// One recorded case: its inputs and the observations it must produce.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// The case name in the recorded corpus.
    name: String,
    /// The host inputs it supplies.
    input: Input,
    /// The observations it must produce.
    expected: Expected,
}

/// The host inputs of one case.
#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Input {
    /// The command arguments.
    args: String,
    /// The tool arguments.
    params: Option<Value>,
    /// Whether the supplied signal is already aborted.
    aborted: bool,
    /// How many times the same callback runs.
    repetitions: usize,
    /// The host rejection the case supplies.
    error: Option<String>,
    /// Whether a reload stays pending until the test releases it.
    hold: bool,
}

impl Default for Input {
    fn default() -> Self {
        Self {
            args: String::new(),
            params: None,
            aborted: false,
            repetitions: 1,
            error: None,
            hold: false,
        }
    }
}

/// The observations one case must produce.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// The calls the host admits, in order.
    calls: Vec<Call>,
    /// Whether the invocation completes or fails.
    outcome: Outcome,
    /// The error the invocation produces.
    error: Option<String>,
    /// Whether the reload stays pending until released.
    #[serde(rename = "pendingBeforeRelease", default)]
    pending_before_release: bool,
    /// The answers the tool returns, in order.
    #[serde(default)]
    results: Vec<Value>,
}

/// Whether a recorded case completes or fails.
#[derive(Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Outcome {
    /// The invocation returned its answer.
    Fulfilled,
    /// The invocation returned an error.
    Rejected,
}

/// One call the host admits.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Call {
    /// The admitted operation.
    operation: String,
    /// The delivered content.
    content: Option<Value>,
    /// The options the delivery carried.
    options: Option<Value>,
}
