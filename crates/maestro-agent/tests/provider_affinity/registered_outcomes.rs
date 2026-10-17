//! Selected and failed hook outcomes through the registered provider in both representations.
use super::{SelectedPayload, Trace, local_input, local_tasks, scenarios};
use crate::support;
use maestro_agent::{Agent, AgentExecution, AgentOptions, AgentState, Local, NoCustomMessages};
use maestro_models::{
    DiagnosticErrorInfo, FauxProviderRegistration, FauxResponseStep, Model, PayloadHookInput,
    ProviderResponse, RegisterFauxProviderOptions, StopReason,
};
use std::{
    rc::Rc,
    sync::{Arc, Mutex},
};

/// Post-operation observations of one recorded prompt.
#[derive(Debug, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Outcome {
    /// Ordered hook and factory steps.
    trace: Vec<String>,
    /// Response metadata the response hook observed.
    responses: Vec<ProviderResponse>,
    /// Factory invocations.
    factory_calls: usize,
    /// Terminal assistant text.
    text: String,
    /// Terminal stop reason.
    stop_reason: StopReason,
    /// Terminal assistant error.
    error_message: Option<String>,
    /// Error retained by the agent.
    state_error: Option<String>,
    /// Whether the agent is still streaming.
    streaming: bool,
    /// Requests the provider received.
    call_count: usize,
    /// Responses still queued.
    pending: usize,
}

/// Which hook rejects its work after suspension.
#[derive(Clone, Copy, PartialEq)]
enum Failure {
    /// Both hooks complete.
    None,
    /// The payload hook rejects.
    Payload,
    /// The response hook rejects.
    Response,
}

/// Shared observations of one hook pair.
struct Hook {
    /// Steps shared with the factory.
    trace: Trace,
    /// Response metadata and the model each step received.
    seen: Mutex<(Vec<ProviderResponse>, Vec<String>)>,
    /// Rejecting hook, if any.
    failure: Failure,
}
impl Hook {
    /// Record a step and the model it received.
    fn step(&self, step: &'static str, model: &Model) {
        self.trace.lock().unwrap().push(step);
        self.seen.lock().unwrap().1.push(model.id.clone());
    }
    /// Replace the whole payload after suspension, unless this hook rejects.
    async fn select(
        &self,
        mut input: PayloadHookInput,
        model: Arc<Model>,
    ) -> Result<PayloadHookInput, DiagnosticErrorInfo> {
        self.step("payload-before", &model);
        tokio::task::yield_now().await;
        if self.failure == Failure::Payload {
            return Err(DiagnosticErrorInfo::error("payload stopped"));
        }
        self.step("payload-after", &model);
        input.payload = serde_json::json!({"text": "11"});
        Ok(input)
    }
    /// Observe response metadata across suspension, unless this hook rejects.
    async fn observe(
        &self,
        response: ProviderResponse,
        model: Arc<Model>,
    ) -> Result<(), DiagnosticErrorInfo> {
        self.seen.lock().unwrap().0.push(response);
        self.step("response-before", &model);
        tokio::task::yield_now().await;
        if self.failure == Failure::Response {
            return Err(DiagnosticErrorInfo::error("response stopped"));
        }
        self.step("response-after", &model);
        Ok(())
    }
}

#[test]
fn registered_agent_hooks_keep_selected_and_failed_outcomes() {
    let prompts: Vec<_> = scenarios()
        .into_iter()
        .filter(|scenario| scenario.input.operation == "prompt")
        .collect();
    assert_eq!(prompts.len(), 3);
    for scenario in prompts {
        let failure = match scenario.input.outcome.as_str() {
            "selected" => Failure::None,
            "payload_failure" => Failure::Payload,
            "response_failure" => Failure::Response,
            other => panic!("unrecorded outcome {other}"),
        };
        let expected: Outcome = serde_json::from_value(scenario.expected).unwrap();
        let api = &scenario.input.outcome;
        assert_eq!(local_outcome(&format!("local-{api}"), failure), expected);
        assert_eq!(
            threaded_outcome(&format!("threaded-{api}"), failure),
            expected
        );
    }
}

/// Register one request whose factory shares the hook trace.
fn registered(api: &str, failure: Failure) -> (FauxProviderRegistration, Hook) {
    let registration = maestro_models::register_faux_provider(RegisterFauxProviderOptions {
        api: Some(api.into()),
        ..Default::default()
    });
    let trace = Trace::default();
    registration.set_responses(vec![FauxResponseStep::Factory(Arc::new(SelectedPayload(
        Arc::clone(&trace),
    )))]);
    let hook = Hook {
        trace,
        seen: Mutex::default(),
        failure,
    };
    (registration, hook)
}

/// Prompt a local agent whose constructor hooks keep `Rc` state across suspension.
fn local_outcome(api: &str, failure: Failure) -> Outcome {
    let (registration, hook) = registered(api, failure);
    let hook = Rc::new(hook);
    let (payload, response) = (Rc::clone(&hook), Rc::clone(&hook));
    let (tasks, spawn) = local_tasks();
    let agent = Rc::new(Agent::<NoCustomMessages, Local>::new_local(AgentOptions {
        spawn: Some(spawn),
        on_payload: Some(Rc::new(move |input, model| {
            let hook = Rc::clone(&payload);
            Box::pin(async move { hook.select(input, model).await })
        })),
        on_response: Some(Rc::new(move |metadata, model| {
            let hook = Rc::clone(&response);
            Box::pin(async move { hook.observe(metadata, model).await })
        })),
        ..Default::default()
    }));
    assert!(hook.trace.lock().unwrap().is_empty());
    *agent.state().write().unwrap().model.write().unwrap() =
        registration.get_model(None).unwrap().clone();
    let observe = agent.prompt(local_input());
    support::run_io(tokio::task::LocalSet::new().run_until(async {
        let running = tokio::task::spawn_local(tasks.next().await.unwrap());
        tasks.next().await.unwrap().await;
        running.await.unwrap();
        observe.await.unwrap();
    }));
    let outcome = outcome(&agent.state().read().unwrap(), &hook, &registration);
    registration.unregister();
    outcome
}

/// Prompt a default agent whose constructor hooks are thread-safe.
fn threaded_outcome(api: &str, failure: Failure) -> Outcome {
    let (registration, hook) = registered(api, failure);
    let hook = Arc::new(hook);
    let (payload, response) = (Arc::clone(&hook), Arc::clone(&hook));
    let agent = Arc::new(Agent::new(AgentOptions {
        spawn: Some(support::scripted::spawner()),
        on_payload: Some(Arc::new(move |input, model| {
            let hook = Arc::clone(&payload);
            Box::pin(async move { hook.select(input, model).await })
        })),
        on_response: Some(Arc::new(move |metadata, model| {
            let hook = Arc::clone(&response);
            Box::pin(async move { hook.observe(metadata, model).await })
        })),
        ..Default::default()
    }));
    assert!(hook.trace.lock().unwrap().is_empty());
    *agent.state().write().unwrap().model.write().unwrap() =
        registration.get_model(None).unwrap().clone();
    support::run_io(async { agent.prompt(local_input()).await }).unwrap();
    let outcome = outcome(&agent.state().read().unwrap(), &hook, &registration);
    registration.unregister();
    outcome
}

/// Read the settled agent, the hook observations and the provider counters.
fn outcome<E: AgentExecution>(
    state: &AgentState<NoCustomMessages, E>,
    hook: &Hook,
    registration: &FauxProviderRegistration,
) -> Outcome {
    let terminal = super::last_assistant(&state.messages().read().unwrap());
    let trace = hook.trace.lock().unwrap().clone();
    let (responses, models) = hook.seen.lock().unwrap().clone();
    let model = registration.get_model(None).unwrap();
    assert!(!models.is_empty() && models.iter().all(|seen| *seen == model.id));
    Outcome {
        factory_calls: trace.iter().filter(|step| **step == "factory").count(),
        trace: trace.into_iter().map(String::from).collect(),
        responses,
        text: super::assistant_text(&terminal),
        stop_reason: terminal.stop_reason,
        error_message: terminal.error_message,
        state_error: state.error_message().map(String::from),
        streaming: state.is_streaming(),
        call_count: registration.state.call_count(),
        pending: registration.get_pending_response_count(),
    }
}
