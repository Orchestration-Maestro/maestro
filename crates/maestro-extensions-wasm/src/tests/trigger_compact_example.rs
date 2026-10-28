//! The shipped compaction-trigger example exercised through both hosts.
use serde::Deserialize;
use serde_json::{Value, json};

use super::catalog_host::Usage;
use super::controlled::Controlled;
use super::documents::{Answer, Ended, document};
use super::host::Harness;
use crate::component_adapter::{Capabilities, Exports};
use crate::{Extension, ExtensionAPI, ExtensionFuture};

#[path = "../../../../examples/extensions/trigger_compact.rs"]
mod trigger_compact;

/// The same factory the shipped component exports.
struct TriggerCompact;

impl Extension for TriggerCompact {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        trigger_compact::trigger_compact(api)
    }
}

/// The compaction result the host reports through a completion callback.
const COMPLETION: &str = r#"{"summary":"summary","firstKeptEntryId":"kept","tokensBefore":1}"#;

/// The recorded corpus these tests consume.
fn corpus() -> Corpus {
    serde_json::from_str(include_str!("trigger_compact_expected.json")).expect("recorded corpus")
}

/// The recorded rows of one group, which a group selection must find at least one of.
fn rows(group: &str) -> Vec<Row> {
    let selected: Vec<Row> = corpus()
        .rows
        .into_iter()
        .filter(|row| row.group == group)
        .collect();
    assert!(!selected.is_empty(), "recorded rows of {group}");
    selected
}

/// The recorded observations these tests consume.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// The recorded cases.
    rows: Vec<Row>,
}

/// One recorded case: the host inputs it supplies and the observations it must produce.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    /// The case name in the recorded corpus.
    name: String,
    /// The group whose test replays this case.
    group: String,
    /// The UI availability the recorded case starts with.
    #[serde(rename = "initialUi")]
    initial_ui: bool,
    /// The input operations, in order.
    steps: Vec<Step>,
    /// What the factory registers, in the order the host received it.
    registrations: Vec<Value>,
    /// The compaction and notification requests, in order.
    effects: Vec<Value>,
    /// The cumulative effect count after each step.
    checkpoints: Vec<usize>,
    /// The outcome of each step.
    outcomes: Vec<Value>,
}

/// One input operation of a recorded case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Step {
    /// The operation name.
    op: String,
    /// The supplied estimate, which is absent when the usage is unknown.
    #[serde(default)]
    tokens: Option<f64>,
    /// A recorded form of unknown usage, which every form reports the same way.
    #[serde(default)]
    usage: Option<String>,
    /// The command arguments.
    #[serde(default)]
    args: Option<String>,
    /// The supplied UI availability.
    #[serde(default)]
    value: Option<bool>,
    /// The admitted request a completion or failure refers to.
    #[serde(default)]
    request: Option<usize>,
    /// The supplied error text or rejection message.
    #[serde(default)]
    message: Option<String>,
    /// The host method a rejection targets.
    #[serde(default)]
    method: Option<String>,
    /// The factory activation a turn belongs to.
    #[serde(default)]
    instance: Option<usize>,
}

/// The two hosts the shipped factory runs on.
trait Host {
    /// Starts `instances` factory activations on one host, in order.
    fn start(instances: usize) -> Result<Self, String>
    where
        Self: Sized;

    /// Delivers one turn with the supplied estimate, or with an unknown one.
    fn turn(&mut self, instance: usize, tokens: Option<f64>) -> Result<(), String>;

    /// Runs the command of one factory activation with `args`.
    fn command(&mut self, instance: usize, args: &str) -> Result<(), String>;

    /// Sets the UI availability the live context reads.
    fn set_ui(&mut self, available: bool);

    /// Supplies the rejection the named host method answers with.
    fn reject(&mut self, method: &str, message: &str);

    /// Delivers a completion to the admitted request at `request`.
    fn complete(&mut self, instance: usize, request: usize) -> Result<(), String>;

    /// Delivers a failure with `message` to the admitted request at `request`.
    fn fail(&mut self, instance: usize, request: usize, message: &str) -> Result<(), String>;

    /// The compaction and notification effects observed so far, in order.
    fn effects(&self) -> Vec<Value>;

    /// The registrations the host received, in the order it received them.
    fn registrations(&self) -> Vec<String>;

    /// The command registrations the host admitted.
    fn commands(&self) -> Vec<(String, Option<String>, bool)>;

    /// Releases every registration and admitted callback of `row`.
    fn release(&mut self, row: &Row) -> Result<(), String>;
}

/// Runs one recorded step on `host`.
fn dispatch<H: Host>(host: &mut H, step: &Step) -> Result<(), String> {
    match step.op.as_str() {
        "turn" => host.turn(
            step.instance.unwrap_or_default(),
            // Every recorded form of unknown usage supplies no estimate.
            if step.usage.is_some() {
                None
            } else {
                step.tokens
            },
        ),
        "command" => host.command(
            step.instance.unwrap_or_default(),
            step.args.as_deref().unwrap_or_default(),
        ),
        "ui" => {
            host.set_ui(step.value.unwrap_or_default());
            Ok(())
        }
        "reject" => {
            host.reject(
                step.method.as_deref().unwrap_or_default(),
                step.message.as_deref().unwrap_or_default(),
            );
            Ok(())
        }
        "complete" => host.complete(
            step.instance.unwrap_or_default(),
            step.request.unwrap_or_default(),
        ),
        "error" => host.fail(
            step.instance.unwrap_or_default(),
            step.request.unwrap_or_default(),
            step.message.as_deref().unwrap_or_default(),
        ),
        other => Err(format!("unknown step {other}")),
    }
}

/// Reads one turn delivery: the handler leaves its event and returns no verdict, or fails.
fn turn_outcome(answer: Answer, event: &Value) -> Result<(), String> {
    match answer.ended {
        Ended::Returned(None) => {
            assert_eq!(answer.event.as_ref(), Some(event), "the turn event");
            Ok(())
        }
        Ended::Returned(Some(result)) => Err(format!("the turn returned {result}")),
        Ended::Failed(message) => Err(message),
    }
}

/// One step's outcome as the recorded documents state it.
fn outcome_of(result: &Result<(), String>) -> Value {
    match result {
        Ok(()) => json!({"ok":null}),
        Err(error) => json!({"error":error}),
    }
}

/// Replays one recorded row and asserts every observation it records.
fn replay<H: Host>(host: &mut H, row: &Row) {
    assert_eq!(
        host.registrations(),
        recorded_registrations(row),
        "{}: registrations",
        row.name
    );
    for command in row
        .registrations
        .iter()
        .filter(|entry| entry.get("command").is_some())
    {
        let name = command["command"].as_str().unwrap().to_owned();
        let description = command["description"].as_str().map(str::to_owned);
        assert!(
            host.commands().contains(&(name, description, false)),
            "{}: command metadata",
            row.name
        );
    }
    for (index, step) in row.steps.iter().enumerate() {
        let outcome = dispatch(host, step);
        assert_eq!(
            outcome_of(&outcome),
            row.outcomes[index],
            "{}: step {index}",
            row.name
        );
        assert_eq!(
            host.effects().as_slice(),
            &row.effects[..row.checkpoints[index]],
            "{}: effects after step {index}",
            row.name
        );
    }
    assert_eq!(host.effects(), row.effects, "{}: effects", row.name);
}

/// The registration lines the recorded row states, in the host's own names.
fn recorded_registrations(row: &Row) -> Vec<String> {
    row.registrations
        .iter()
        .map(|entry| match entry.get("event") {
            Some(event) => format!("register event {}", event.as_str().unwrap()),
            None => format!("register command {}", entry["command"].as_str().unwrap()),
        })
        .collect()
}

/// The names the host keeps its registrations under, in registration order.
fn registration_names(row: &Row) -> Vec<String> {
    row.registrations
        .iter()
        .map(|entry| match entry.get("event") {
            Some(event) => format!("event {}", event.as_str().unwrap()),
            None => format!("command {}", entry["command"].as_str().unwrap()),
        })
        .collect()
}

/// The names of every registration and admitted callback the host keeps, in release order.
fn release_order(row: &Row) -> Vec<String> {
    let mut names = registration_names(row);
    let mut request = 0;
    for effect in &row.effects {
        if effect.get("compact").is_some() {
            names.push(format!("compact {request} complete"));
            names.push(format!("compact {request} error"));
            request += 1;
        }
    }
    names
}

/// The compaction and notification effects the host's lines record, in order.
fn observed_effects(lines: &[String]) -> Vec<Value> {
    lines.iter().filter_map(|line| effect(line)).collect()
}

/// The effect one host line records, if it records one.
fn effect(line: &str) -> Option<Value> {
    if let Some(request) = line.strip_prefix("compact admission ") {
        let request: Value = serde_json::from_str(request).ok()?;
        let instructions = match &request["instructions"] {
            instructions if instructions["present"] == true => instructions["value"].clone(),
            _ => Value::Null,
        };
        return Some(json!({"compact":{
            "instructions":instructions,
            "complete":request["complete"],
            "error":request["error"],
        }}));
    }
    notification(&super::ui_host::receipt(line)?)
}

/// The notification effect a recorded UI request carries; other requests record none.
fn notification(receipt: &super::ui_host::Receipt) -> Option<Value> {
    if receipt.method != "notify" {
        return None;
    }
    let operands = &receipt.operands;
    Some(json!({"notify":{
        "message":operands[0],
        "kind":operands[1].as_str().unwrap_or_default().to_lowercase(),
    }}))
}

/// The in-process host and the keys the shipped factory registered.
struct ControlledHost {
    /// The host that recorded what the factory asked for.
    host: Controlled,
    /// The adapter serving the factory's callbacks.
    exports: Exports<Controlled>,
    /// The `turn_end` callback of each started factory.
    turns: Vec<u32>,
    /// The `trigger-compact` command of each started factory.
    commands: Vec<u32>,
}

impl Host for ControlledHost {
    fn start(instances: usize) -> Result<Self, String> {
        let host = Controlled::default();
        let exports = Exports::new(host.clone());
        let (mut turns, mut commands) = (Vec::new(), Vec::new());
        super::block_on(async {
            for _ in 0..instances {
                exports.start::<TriggerCompact>().await?;
                turns.push(host.observed().callback("event turn_end")?);
                commands.push(host.observed().callback("command trigger-compact")?);
            }
            Ok::<(), String>(())
        })?;
        Ok(Self {
            host,
            exports,
            turns,
            commands,
        })
    }

    fn turn(&mut self, instance: usize, tokens: Option<f64>) -> Result<(), String> {
        self.host.observe().catalog.data.usage = Some(Usage {
            tokens,
            context_window: 200_000.0,
            percent: None,
        });
        let event = document("turn_end");
        let key = self.turns[instance];
        let outcome = super::block_on(self.exports.invoke_event(
            key,
            event.to_string(),
            Capabilities {
                ctx: self.host.context("/work"),
                signal: None,
            },
        ))?;
        turn_outcome(Answer::of(super::guest_family::delivery(outcome))?, &event)
    }

    fn command(&mut self, instance: usize, args: &str) -> Result<(), String> {
        let key = self.commands[instance];
        super::block_on(self.exports.invoke_command(
            key,
            args.to_owned(),
            self.host.command_context("/work"),
        ))
    }

    fn set_ui(&mut self, available: bool) {
        self.host.observe().catalog.data.has_ui = available;
    }

    fn reject(&mut self, method: &str, message: &str) {
        match method {
            "compact" => self.host.observe().actions.error = Some(message.to_owned()),
            "notify" => self
                .host
                .observe()
                .entry("ui-control", Some(&json!({"error":message}).to_string())),
            other => panic!("unknown rejected method {other}"),
        }
    }

    fn complete(&mut self, instance: usize, request: usize) -> Result<(), String> {
        let _ = instance;
        let key = self
            .host
            .observed()
            .callback(&format!("compact {request} complete"))?;
        self.exports.invoke_compaction_complete(
            key,
            COMPLETION,
            &super::bus_host::Emitter { key: 0 },
        )
    }

    fn fail(&mut self, instance: usize, request: usize, message: &str) -> Result<(), String> {
        let _ = instance;
        let key = self
            .host
            .observed()
            .callback(&format!("compact {request} error"))?;
        self.exports.invoke_compaction_error(
            key,
            message.to_owned(),
            &super::bus_host::Emitter { key: 0 },
        )
    }

    fn effects(&self) -> Vec<Value> {
        observed_effects(&self.host.observed().transcript)
    }

    fn registrations(&self) -> Vec<String> {
        registered(&self.host.observed().transcript)
    }

    fn commands(&self) -> Vec<(String, Option<String>, bool)> {
        self.host.observed().registrations.commands.clone()
    }

    fn release(&mut self, row: &Row) -> Result<(), String> {
        super::example_support::release_all(&self.host, &release_order(row));
        Ok(())
    }
}

/// Releases one instance's registrations and admitted callbacks, returning its request count.
fn released(started: &mut Instance) -> Result<usize, String> {
    let requests = started.harness.store.data().observed.compactions;
    for key in [started.turn, started.command] {
        super::block_on(started.harness.release(key)).map_err(|error| error.to_string())?;
    }
    for request in 0..requests {
        for kind in ["complete", "error"] {
            release_admitted(started, request, kind)?;
        }
    }
    Ok(requests)
}

/// Releases one admitted callback of an instance.
fn release_admitted(started: &mut Instance, request: usize, kind: &str) -> Result<(), String> {
    let key = started
        .harness
        .callback(&format!("compact {request} {kind}"))
        .map_err(|error| error.to_string())?;
    super::block_on(started.harness.release(key)).map_err(|error| error.to_string())
}

/// The registration lines of a recorded transcript.
fn registered(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .filter(|line| line.starts_with("register "))
        .cloned()
        .collect()
}

/// One instantiated shipped component with the keys it registered.
struct Instance {
    /// The running component.
    harness: Harness,
    /// The registered `turn_end` callback.
    turn: u32,
    /// The registered `trigger-compact` command.
    command: u32,
}

/// The built component, instantiated once per factory activation.
struct ComponentHost {
    /// The instances in start order.
    instances: Vec<Instance>,
}

impl ComponentHost {
    /// Delivers a completion or a failure to the admitted request at `request`.
    fn notify(
        &mut self,
        instance: usize,
        request: usize,
        payload: &str,
        complete: bool,
    ) -> Result<(), String> {
        let kind = if complete { "complete" } else { "error" };
        let started = &mut self.instances[instance];
        let key = started
            .harness
            .callback(&format!("compact {request} {kind}"))
            .map_err(|error| error.to_string())?;
        super::block_on(super::host::notify_compaction(
            &mut started.harness,
            key,
            payload,
            complete,
        ))
    }
}

impl Host for ComponentHost {
    fn start(instances: usize) -> Result<Self, String> {
        let path = super::build::example_component("maestro-extensions-wasm", "trigger_compact")?;
        let (engine, component) =
            super::build::compile(&path).map_err(|error| error.to_string())?;
        let mut started = Vec::new();
        for _ in 0..instances {
            let harness = super::block_on(Harness::from_compiled(&engine, &component))
                .map_err(|error| error.to_string())?;
            let turn = harness
                .callback("event turn_end")
                .map_err(|error| error.to_string())?;
            let command = harness
                .callback("command trigger-compact")
                .map_err(|error| error.to_string())?;
            started.push(Instance {
                harness,
                turn,
                command,
            });
        }
        Ok(Self { instances: started })
    }

    fn turn(&mut self, instance: usize, tokens: Option<f64>) -> Result<(), String> {
        let started = &mut self.instances[instance];
        started.harness.observed().catalog.data.usage = Some(Usage {
            tokens,
            context_window: 200_000.0,
            percent: None,
        });
        let event = document("turn_end");
        let capabilities = super::host_family::guest::Capabilities {
            ctx: started
                .harness
                .ordinary("/work")
                .map_err(|error| error.to_string())?,
            signal: None,
        };
        let exports = started.harness.exports.clone();
        let key = started.turn;
        let text = event.to_string();
        let outcome = super::block_on(started.harness.store.run_concurrent(async |accessor| {
            exports
                .call_invoke_event(accessor, super::host::borrow(key), text, capabilities)
                .await
        }))
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())??;
        turn_outcome(Answer::of(super::host_family::delivery(outcome))?, &event)
    }

    fn command(&mut self, instance: usize, args: &str) -> Result<(), String> {
        let started = &mut self.instances[instance];
        super::example_support::invoke_command_on_component(
            &mut started.harness,
            started.command,
            args,
            false,
        )
    }

    fn set_ui(&mut self, available: bool) {
        for started in &mut self.instances {
            started.harness.observed().catalog.data.has_ui = available;
        }
    }

    fn reject(&mut self, method: &str, message: &str) {
        for started in &mut self.instances {
            match method {
                "compact" => started.harness.observed().actions.error = Some(message.to_owned()),
                "notify" => started
                    .harness
                    .observed()
                    .entry("ui-control", Some(&json!({"error":message}).to_string())),
                other => panic!("unknown rejected method {other}"),
            }
        }
    }

    fn complete(&mut self, instance: usize, request: usize) -> Result<(), String> {
        self.notify(instance, request, COMPLETION, true)
    }

    fn fail(&mut self, instance: usize, request: usize, message: &str) -> Result<(), String> {
        self.notify(instance, request, message, false)
    }

    fn effects(&self) -> Vec<Value> {
        self.instances
            .iter()
            .flat_map(|started| observed_effects(&started.harness.store.data().observed.transcript))
            .collect()
    }

    fn registrations(&self) -> Vec<String> {
        self.instances
            .iter()
            .flat_map(|started| registered(&started.harness.store.data().observed.transcript))
            .collect()
    }

    fn commands(&self) -> Vec<(String, Option<String>, bool)> {
        self.instances
            .iter()
            .flat_map(|started| {
                started
                    .harness
                    .store
                    .data()
                    .observed
                    .registrations
                    .commands
                    .clone()
            })
            .collect()
    }

    fn release(&mut self, row: &Row) -> Result<(), String> {
        let recorded = row
            .effects
            .iter()
            .filter(|effect| effect.get("compact").is_some())
            .count();
        let mut admitted = 0;
        for started in &mut self.instances {
            admitted += released(started)?;
        }
        assert_eq!(admitted, recorded, "{}: admitted requests", row.name);
        for started in &self.instances {
            assert_eq!(
                started.harness.store.data().observed.live,
                0,
                "the host kept nothing it lent"
            );
        }
        Ok(())
    }
}

/// Exactly `turn_end`, then `trigger-compact`, with no completion and no other registration.
#[test]
fn trigger_compact_registers_hook_and_command() -> Result<(), String> {
    let recorded = rows("crossings");
    let row = recorded
        .iter()
        .find(|row| row.effects.is_empty())
        .expect("a recorded case without requests");
    let mut host = ControlledHost::start(1)?;
    assert_eq!(host.registrations(), recorded_registrations(row));
    assert_eq!(
        host.commands(),
        [(
            "trigger-compact".to_owned(),
            Some("Trigger compaction immediately".to_owned()),
            false
        )]
    );
    host.release(row)
}

/// Every recorded crossing, including the boundaries, fractions, zero and recrossing.
#[test]
fn trigger_compact_crossings_keep_previous_usage() -> Result<(), String> {
    for row in rows("crossings") {
        let mut host = ControlledHost::start(1)?;
        replay(&mut host, &row);
        host.release(&row)?;
    }
    Ok(())
}

/// A form of unknown usage neither seeds nor erases the last known estimate.
#[test]
fn trigger_compact_unknown_usage_keeps_last_estimate() -> Result<(), String> {
    for row in rows("unknown") {
        let mut host = ControlledHost::start(1)?;
        replay(&mut host, &row);
        host.release(&row)?;
    }
    Ok(())
}

/// The command forwards trimmed instructions and leaves the crossing state alone.
#[test]
fn trigger_compact_command_preserves_instructions_and_crossing_state() -> Result<(), String> {
    for row in rows("commands") {
        let mut host = ControlledHost::start(1)?;
        replay(&mut host, &row);
        host.release(&row)?;
    }
    Ok(())
}

/// Each notification reads the UI availability the context has when it reports.
#[test]
fn trigger_compact_notifications_follow_current_ui() -> Result<(), String> {
    for row in rows("notifications") {
        let mut host = ControlledHost::start(1)?;
        host.set_ui(row.initial_ui);
        replay(&mut host, &row);
        host.release(&row)?;
    }
    Ok(())
}

/// A rejected host operation reaches its caller unchanged and requests nothing further.
#[test]
fn trigger_compact_host_rejections_preserve_prior_requests() -> Result<(), String> {
    for row in rows("rejections") {
        let mut host = ControlledHost::start(1)?;
        host.set_ui(row.initial_ui);
        replay(&mut host, &row);
        host.release(&row)?;
    }
    Ok(())
}

/// Each factory activation keeps its own last known estimate.
#[test]
fn trigger_compact_factory_instances_keep_separate_usage() -> Result<(), String> {
    for row in rows("instances") {
        let mut host = ControlledHost::start(row.registrations.len() / 2)?;
        replay(&mut host, &row);
        host.release(&row)?;
    }
    Ok(())
}

/// The built component produces the recorded requests for every case.
#[test]
fn trigger_compact_component_matches_host_requests() -> Result<(), String> {
    for row in corpus().rows {
        let instances = row.registrations.len() / 2;
        let mut host = ComponentHost::start(instances)?;
        host.set_ui(row.initial_ui);
        replay(&mut host, &row);
        host.release(&row)?;
    }
    Ok(())
}
