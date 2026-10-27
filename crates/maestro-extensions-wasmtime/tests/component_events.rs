//! Recorded hook outcomes replayed through built components and the native runner.
//!
//! Each recorded case names its handlers' behavior; a pool of loaded hook components installs
//! those handlers through their `hooks` command, the native runner dispatches the event, and the
//! observed outcome is compared with the recording. The recording keeps open-object values
//! outside the declared event and result types; each case states how its outcome is projected
//! onto the declared types before the comparison.
#![cfg(test)]
use super::components::{TracedBus, hooks, local};
use maestro_extensions::{
    Cancellation, CommandContextOperations, ExtensionCommandContextActions,
    ExtensionContextActions, ExtensionExecution, ExtensionRunner, Local, create_extension_runtime,
    load_extension_from_factory,
    types::{ExtensionHandle, ExtensionRuntime, ProviderActions},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc, sync::RwLock};
use tokio::sync::Notify;

mod cases;
mod input;
mod sessions;
mod values;

/// Most instances one recorded case loads.
const POOL: usize = 32;

/// One recorded case: its group, complete input and recorded outcome.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// The group the case belongs to.
    name: String,
    /// The complete recorded input.
    input: Value,
    /// The recorded outcome.
    output: Value,
}

/// Loaded hook components and the controls every case shares.
pub(super) struct Harness {
    /// Loaded instances, labelled `<extension:i>`.
    owners: Vec<ExtensionHandle<Local>>,
    /// The runtime the instances share.
    runtime: Rc<ExtensionRuntime<Local>>,
    /// The bus whose trace channel the components report on.
    pub(super) bus: TracedBus,
    /// Holds a component waiting for idle until the case releases it.
    pub(super) gate: Gate,
    /// The cancellation a context's abort cancels.
    pub(super) cancellation: Rc<RefCell<Option<Cancellation>>>,
}

/// A wait the case observes entering and releases.
#[derive(Clone, Default)]
pub(super) struct Gate {
    /// Notified when a component starts waiting.
    pub(super) entered: Rc<Notify>,
    /// Releases the waiting component.
    pub(super) release: Rc<Notify>,
}

/// One case's runner and the diagnostics its error listener received.
pub(super) struct Run {
    /// Runner over the case's instances.
    pub(super) runner: Rc<ExtensionRunner<Local>>,
    /// Received diagnostics, without engine stack text.
    errors: Rc<RefCell<Vec<Value>>>,
}

impl Run {
    /// The diagnostics received so far.
    pub(super) fn errors(&self) -> Value {
        Value::Array(self.errors.borrow().clone())
    }
}

/// The label of instance `index`, which is also its extension path.
pub(super) fn label(index: usize) -> String {
    format!("<extension:{index}>")
}

impl Harness {
    /// Loads the pool concurrently, each instance on its own Store.
    async fn load() -> Self {
        let path = std::env::var("MAESTRO_HOOK_COMPONENT").unwrap();
        let runtime = create_extension_runtime::<Local>();
        let bus = TracedBus::new();
        let loads = (0..POOL).map(|index| {
            load_extension_from_factory(
                crate::artifact(path.clone()),
                "/work".into(),
                bus.controller.for_extension(),
                runtime.clone(),
                Some(label(index)),
            )
        });
        let owners = futures_util::future::join_all(loads).await;
        Self {
            owners: owners.into_iter().map(Result::unwrap).collect(),
            runtime,
            bus,
            gate: Gate::default(),
            cancellation: Rc::default(),
        }
    }

    /// A runner over the first `count` instances, each with only `handlers[i]` registered.
    ///
    /// `handlers[i]` lists the steps of each handler instance `i` registers for `event`.
    pub(super) async fn prepare(&self, event: &str, handlers: &[Vec<Value>]) -> Run {
        let owners = self.owners[..handlers.len()].to_vec();
        for (index, owner) in owners.iter().enumerate() {
            let mut owner = owner.write().unwrap();
            owner.handlers.clear();
            owner.path = label(index);
        }
        let run = self.run(owners);
        for (index, steps) in handlers.iter().enumerate() {
            self.install(&run, index, event, steps).await;
        }
        run
    }

    /// Registers handlers with the given steps on instance `index`.
    pub(super) async fn install(&self, run: &Run, index: usize, event: &str, handlers: &[Value]) {
        let handlers: Vec<_> = handlers
            .iter()
            .map(|steps| json!({ "event": event, "steps": steps }))
            .collect();
        let request = json!({ "handlers": handlers });
        hooks(&self.owners[index], &run.runner, &request)
            .await
            .unwrap();
    }

    /// Instance `index`.
    pub(super) fn owner(&self, index: usize) -> &ExtensionHandle<Local> {
        &self.owners[index]
    }

    /// A bound runner whose abort cancels the case's cancellation and whose idle wait is gated.
    fn run(&self, owners: Vec<ExtensionHandle<Local>>) -> Run {
        let runner = Rc::new(ExtensionRunner::<Local>::new(
            Rc::new(RwLock::new(owners)),
            self.runtime.clone(),
            super::session::runner_session("/work").0,
            Rc::new(|_| {}),
            maestro_theme::LiveTheme::default(),
        ));
        let cancellation = self.cancellation.clone();
        runner
            .bind_core(
                Local::unbound_actions(),
                ExtensionContextActions {
                    get_system_prompt: Rc::new(|| Ok("context prompt".into())),
                    abort: Rc::new(move || {
                        cancellation.borrow().as_ref().map(Cancellation::abort);
                        Ok(())
                    }),
                    ..Default::default()
                },
                ProviderActions::default(),
            )
            .unwrap();
        let gate = self.gate.clone();
        runner.bind_command_context(Some(ExtensionCommandContextActions::new(
            CommandContextOperations {
                wait_for_idle: Rc::new(move || {
                    let gate = gate.clone();
                    Box::pin(async move {
                        gate.entered.notify_one();
                        gate.release.notified().await;
                        Ok(())
                    })
                }),
                ..super::context_actions::silent_command_operations()
            },
        )));
        let errors = Rc::new(RefCell::new(Vec::new()));
        let received = errors.clone();
        let _keep = runner.on_error(Rc::new(move |error| {
            received.borrow_mut().push(json!({
                "extensionPath": error.extension_path,
                "event": error.event,
                "error": error.error,
            }));
            Ok(())
        }));
        Run { runner, errors }
    }
}

/// The recorded outcome without engine stack presence, which no component error carries.
fn without_stacks(mut output: Value) -> Value {
    if let Some(errors) = output.get_mut("errors").and_then(Value::as_array_mut) {
        for error in errors {
            error.as_object_mut().unwrap().shift_remove("stack");
        }
    }
    output
}

#[test]
fn component_hooks_replay_recorded_domain_outcomes() {
    let cases: Vec<Case> = serde_json::from_str(include_str!("fixtures/hook_corpus.json")).unwrap();
    assert_eq!(cases.len(), 104);
    // The authored stack case belongs to the diagnostic owner that carries supplied stacks.
    assert_eq!(cases[103].name, "authored-stack");
    local(async {
        let harness = Harness::load().await;
        for (index, case) in cases[..103].iter().enumerate() {
            let recorded = without_stacks(case.output.clone());
            let (observed, expected) =
                cases::replay(&harness, index, &case.name, &case.input, recorded).await;
            assert_eq!(
                observed, expected,
                "case {index} {} {}",
                case.name, case.input
            );
            assert!(
                harness.bus.take().is_empty(),
                "case {index} left trace values"
            );
        }
    });
}

#[test]
fn component_handler_presence_tracks_registration() {
    local(async {
        let harness = Harness::load().await;
        let empty = harness.prepare("input", &[vec![]]).await;
        assert!(!empty.runner.has_handlers("input"));
        assert!(!empty.runner.has_handlers("tool_call"));
        harness.install(&empty, 0, "input", &[json!([])]).await;
        assert!(empty.runner.has_handlers("input"));
        assert!(!empty.runner.has_handlers("tool_call"));
        harness.install(&empty, 0, "tool_call", &[json!([])]).await;
        assert!(empty.runner.has_handlers("tool_call"));
        assert!(!empty.runner.has_handlers("agent_end"));
        let cleared = harness.prepare("input", &[vec![]]).await;
        assert!(!cleared.runner.has_handlers("input"));
        assert!(!cleared.runner.has_handlers("tool_call"));
    });
}

#[test]
fn component_tool_results_consume_prior_content() {
    local(async {
        let harness = Harness::load().await;
        let run = harness
            .prepare(
                "tool_result",
                &[
                    vec![json!([{"suffix":"first"}])],
                    vec![json!([{"suffix":"second"}])],
                ],
            )
            .await;
        let event = cases::typed(json!({
            "toolName":"read", "toolCallId":"call", "input":{},
            "content":[{"type":"text", "text":"base"}],
            "details":{"initial":true}, "isError":false,
        }));
        let result = run.runner.emit_tool_result(&event).await.unwrap().unwrap();
        assert_eq!(
            cases::encoded(&result),
            json!({
                "content":[
                    {"type":"text", "text":"base"},
                    {"type":"text", "text":"first"},
                    {"type":"text", "text":"second"},
                ],
                "details":{"initial":true}, "isError":false,
            })
        );
        assert_eq!(run.errors(), json!([]));
        assert!(harness.bus.take().is_empty());
    });
}
