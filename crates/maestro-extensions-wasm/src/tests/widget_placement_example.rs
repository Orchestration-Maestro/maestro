//! The shipped widget-placement factory exercised through registration and invocation.
use serde::Deserialize;
use serde_json::Value;

use super::controlled::Controlled;
use super::documents::{Answer, Ended};
use super::host::Harness;
use super::host_family::guest;
use super::ui_host::{Receipt, WidgetOptions, receipts, widget};
use crate::component_adapter::{Capabilities, Exports, release};
use crate::{Extension, ExtensionAPI, ExtensionFuture};

#[path = "../../../../examples/extensions/widget_placement.rs"]
mod widget_placement;

/// The same factory exported by the shipped component.
struct WidgetPlacement;

impl Extension for WidgetPlacement {
    fn load(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
        widget_placement::widget_placement_extension(api)
    }
}

/// One guest host the shipped factory runs on.
trait WidgetHost {
    /// Restores the UI bank a fresh host starts each corpus case from.
    fn reset(&mut self);
    /// Supplies the host UI availability read by the next invocation.
    fn set_available(&mut self, available: bool);
    /// Resolves a different UI binding after the first accepted request.
    fn rebind_after_first(&mut self);
    /// Rejects the request at `index`, counted from zero, with `error`.
    fn reject_at(&mut self, index: usize, error: &str);
    /// Delivers one session start to `handler` and reports how it ended.
    async fn session_start(&mut self, handler: u32) -> Result<Answer, String>;
    /// The UI requests the host recorded, in order.
    fn requests(&self) -> Vec<Receipt>;
}

/// A controlled host with the exports the factory registers its callbacks in.
struct ControlledHost {
    /// What the host observed and controls.
    host: Controlled,
    /// The exports of the factory.
    exports: Exports<Controlled>,
}

impl ControlledHost {
    /// The started factory with the handler it registered for session starts.
    async fn started() -> Result<(Self, u32), String> {
        let (host, handler, exports) =
            super::example_support::registered::<WidgetPlacement>("event session_start").await?;
        Ok((Self { host, exports }, handler))
    }

    /// Releases `handler` and asserts the host keeps nothing lent.
    fn finish(&self, handler: u32) {
        release(handler);
        assert_eq!(
            self.host.observed().live,
            0,
            "resources the host still lends"
        );
    }
}

impl WidgetHost for ControlledHost {
    fn reset(&mut self) {
        self.host.observe().ui.reset();
    }

    fn set_available(&mut self, available: bool) {
        self.host.observe().catalog.data.has_ui = available;
    }

    fn rebind_after_first(&mut self) {
        self.host.observe().ui.switch_after_first(1);
    }

    fn reject_at(&mut self, index: usize, error: &str) {
        self.host.observe().ui.reject_at(index, error);
    }

    async fn session_start(&mut self, handler: u32) -> Result<Answer, String> {
        let capabilities = Capabilities {
            ctx: self.host.context("/controlled"),
            signal: None,
        };
        let outcome = self
            .exports
            .invoke_event(handler, session_start_event().to_string(), capabilities)
            .await?;
        Answer::of(super::guest_family::delivery(outcome))
    }

    fn requests(&self) -> Vec<Receipt> {
        receipts(&self.host.observed().transcript)
    }
}

/// The recorded requests and outcomes consumed by these tests.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Event names the factory registers.
    registrations: Vec<String>,
    /// Cases, each one sequence of invocations of the one handler.
    cases: Vec<Case>,
}

/// One sequence of invocations of the one registered handler.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Label used in assertion messages.
    name: String,
    /// Invocations in order.
    steps: Vec<Step>,
}

/// One invocation: the host state it supplies and what it must produce.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Step {
    /// Host state supplied before the invocation.
    input: Input,
    /// Requests and outcome the invocation must produce.
    expected: Expected,
}

/// Host state one invocation supplies.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Input {
    /// Whether the host reports UI availability.
    #[serde(rename = "hasUI")]
    has_ui: bool,
    /// Whether the binding changes after the first accepted request.
    #[serde(default)]
    rebind_after_above: bool,
    /// Index of the rejected request, counted from zero.
    #[serde(default)]
    reject_at: Option<usize>,
    /// Message the rejected request fails with.
    #[serde(default)]
    error: Option<String>,
}

/// What one invocation must produce.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// Requests in the order the host must receive them.
    calls: Vec<Call>,
    /// How the callback must end.
    outcome: Outcome,
}

/// One recorded request.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Call {
    /// Identity of the acquisition the request is made on.
    owner: usize,
    /// Widget key.
    key: String,
    /// Literal lines of the widget.
    lines: Vec<String>,
    /// Requested placement; absent keeps the host default.
    options: Option<Options>,
}

/// Requested widget options.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Options {
    /// Authored placement word.
    placement: Placement,
}

/// Authored placement words.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum Placement {
    /// Below the editor.
    BelowEditor,
}

impl Placement {
    /// The word the host records for this placement.
    fn recorded(&self) -> &'static str {
        match self {
            Self::BelowEditor => "BelowEditor",
        }
    }
}

/// How the callback must end.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase", tag = "kind")]
enum Outcome {
    /// It returned without a result.
    Fulfilled,
    /// It failed with this message.
    Rejected {
        /// Message the host supplies for the rejected request.
        error: String,
    },
}

/// The corpus every test consumes.
fn corpus() -> Corpus {
    serde_json::from_str(include_str!("widget_placement_expected.json")).unwrap()
}

/// The event a session start delivers.
fn session_start_event() -> Value {
    super::documents::document("session_start")
}

/// The request operands the host must record for `call`.
fn expected_widget(call: &Call) -> Value {
    let options = call.options.as_ref().map(|options| WidgetOptions {
        placement: Some(options.placement.recorded().to_owned()),
    });
    widget(&call.key, Some(&call.lines), options)
}

/// The answer a step must produce.
fn expected_answer(outcome: &Outcome) -> Answer {
    let event = session_start_event();
    match outcome {
        Outcome::Fulfilled => Answer::returned(&event, None),
        Outcome::Rejected { error } => Answer {
            event: Some(event),
            ended: Ended::Failed(error.clone()),
        },
    }
}

/// Runs the named corpus cases on `host` through the one registered handler.
async fn run(host: &mut impl WidgetHost, handler: u32, names: &[&str]) -> Result<(), String> {
    let corpus = corpus();
    let cases: Vec<&Case> = corpus
        .cases
        .iter()
        .filter(|case| names.contains(&case.name.as_str()))
        .collect();
    assert_eq!(cases.len(), names.len(), "the cases this test names");
    for case in cases {
        host.reset();
        for step in &case.steps {
            run_step(host, handler, &case.name, step).await?;
        }
    }
    Ok(())
}

/// Delivers one step and asserts the requests and outcome it must produce.
async fn run_step(
    host: &mut impl WidgetHost,
    handler: u32,
    name: &str,
    step: &Step,
) -> Result<(), String> {
    host.set_available(step.input.has_ui);
    if step.input.rebind_after_above {
        host.rebind_after_first();
    }
    if let Some(index) = step.input.reject_at {
        let error = step
            .input
            .error
            .as_deref()
            .ok_or("a rejected step supplies no message")?;
        host.reject_at(index, error);
    }
    let before = host.requests().len();
    let answer = host.session_start(handler).await?;
    let produced = host.requests().split_off(before);
    assert_requests(&produced, &step.expected.calls, name);
    assert_eq!(
        answer,
        expected_answer(&step.expected.outcome),
        "{name} outcome"
    );
    Ok(())
}

/// Asserts the requests one invocation recorded against the expected ones.
fn assert_requests(produced: &[Receipt], calls: &[Call], name: &str) {
    assert_eq!(produced.len(), calls.len(), "{name} requests");
    for (receipt, call) in produced.iter().zip(calls) {
        assert_eq!(receipt.key, call.owner, "{name} owner");
        assert_eq!(receipt.method, "setWidget", "{name} method");
        assert_eq!(receipt.operands, expected_widget(call), "{name} operands");
    }
}

/// Only the session start handler is registered, and startup requests no widget.
#[test]
fn widget_placement_registers_only_session_start() -> Result<(), String> {
    super::block_on(async {
        let (host, _exports) = super::example_support::start::<WidgetPlacement>().await?;
        let names: Vec<String> = corpus()
            .registrations
            .iter()
            .map(|name| format!("event {name}"))
            .collect();
        for name in &names {
            host.observed().callback(name)?;
        }
        assert!(host.observed().tools.is_empty(), "authored tools");
        assert!(
            receipts(&host.observed().transcript).is_empty(),
            "widget requests during startup"
        );
        super::example_support::release_all(&host, &names);
        Ok(())
    })
}

/// Each start reads the live UI availability and requests both literal widgets.
#[test]
fn widget_placement_keeps_literals_and_rechecks_ui() -> Result<(), String> {
    run_controlled(&[
        "headless_start",
        "interactive_start",
        "ui_presence_changes_between_starts",
        "ui_binding_changes_after_above",
    ])
}

/// A rejected request ends the invocation with its own message and attempts nothing further.
#[test]
fn widget_placement_stops_on_host_rejection() -> Result<(), String> {
    run_controlled(&["above_request_rejected", "below_request_rejected"])
}

/// Runs the named corpus cases on a controlled host and releases its handler.
fn run_controlled(names: &[&str]) -> Result<(), String> {
    super::block_on(async {
        let (mut host, handler) = ControlledHost::started().await?;
        run(&mut host, handler, names).await?;
        host.finish(handler);
        Ok(())
    })
}

impl WidgetHost for Harness {
    fn reset(&mut self) {
        self.observed().ui.reset();
    }

    fn set_available(&mut self, available: bool) {
        self.observed().catalog.data.has_ui = available;
    }

    fn rebind_after_first(&mut self) {
        self.observed().ui.switch_after_first(1);
    }

    fn reject_at(&mut self, index: usize, error: &str) {
        self.observed().ui.reject_at(index, error);
    }

    async fn session_start(&mut self, handler: u32) -> Result<Answer, String> {
        let capabilities = guest::Capabilities {
            ctx: self
                .ordinary("/controlled")
                .map_err(|error| error.to_string())?,
            signal: None,
        };
        let exports = self.exports.clone();
        let event = session_start_event().to_string();
        let run = self
            .store
            .run_concurrent(async |accessor| {
                exports
                    .call_invoke_event(accessor, super::host::borrow(handler), event, capabilities)
                    .await
            })
            .await
            .map_err(|error| error.to_string())?;
        let called = run.map_err(|error| error.to_string())?;
        Answer::of(super::host_family::delivery(called?))
    }

    fn requests(&self) -> Vec<Receipt> {
        receipts(&self.store.data().observed.transcript)
    }
}

/// The built single-copy example requests the same widgets across the component interface.
#[test]
fn widget_placement_component_runs_session_start() -> Result<(), String> {
    super::block_on(async {
        let path = super::build::example_component("widget_placement")?;
        let mut harness = Harness::start(&path)
            .await
            .map_err(|error| error.to_string())?;
        let handler = harness
            .callback("event session_start")
            .map_err(|error| error.to_string())?;
        assert!(harness.observed().tools.is_empty(), "authored tools");
        assert!(
            receipts(&harness.store.data().observed.transcript).is_empty(),
            "widget requests during startup"
        );
        let corpus = corpus();
        let names: Vec<&str> = corpus.cases.iter().map(|case| case.name.as_str()).collect();
        run(&mut harness, handler, &names).await?;
        harness
            .release(handler)
            .await
            .map_err(|error| error.to_string())?;
        assert_eq!(harness.observed().live, 0, "resources the host still lends");
        Ok(())
    })
}
