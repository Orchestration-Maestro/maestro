//! Family-correct model hooks through owned agent execution.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::support;
use maestro_agent::{Agent, AgentOptions, Local, NoCustomMessages, PromptInput};
use maestro_models::{AssistantContent, FauxResponseStep, RegisterFauxProviderOptions};
use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    sync::{Arc, Mutex},
};

mod registered_outcomes;

/// Recorded agent scenarios for registered provider hooks.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Scenarios {
    /// Scenarios in recorded order.
    rows: Vec<Scenario>,
}
/// One recorded operation, hook outcome and its post-operation observations.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Scenario {
    /// Operation and hook outcome selecting the driver.
    input: ScenarioInput,
    /// Observations decoded by the consuming test.
    expected: serde_json::Value,
}
/// Selects the operation sequence and the hook outcome.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ScenarioInput {
    /// Agent operation sequence.
    operation: String,
    /// Hook outcome.
    outcome: String,
}
/// Load the recorded scenarios.
fn scenarios() -> Vec<Scenario> {
    serde_json::from_str::<Scenarios>(include_str!("fixtures/agent_provider_hooks.json"))
        .unwrap()
        .rows
}

#[test]
fn local_agent_constructor_hooks_reach_registered_dispatch() {
    let registration = maestro_models::register_faux_provider(RegisterFauxProviderOptions {
        api: Some("local-agent-constructor".into()),
        ..Default::default()
    });
    registration.set_responses(vec![FauxResponseStep::Factory(Arc::new(
        SelectedPayload::default(),
    ))]);
    let (tasks, spawn) = local_tasks();
    let observations = Rc::new(Cell::new(0));
    let observed = Rc::clone(&observations);
    let agent = Rc::new(Agent::<NoCustomMessages, Local>::new_local(AgentOptions {
        spawn: Some(spawn),
        on_payload: Some(payload_hook("11")),
        on_response: Some(Rc::new(move |_, _| {
            let observed = Rc::clone(&observed);
            Box::pin(async move {
                tokio::task::yield_now().await;
                observed.set(observed.get() + 1);
                Ok(())
            })
        })),
        ..Default::default()
    }));
    *agent.state().write().unwrap().model.write().unwrap() =
        registration.get_model(None).unwrap().clone();
    let observe = agent.prompt(PromptInput::Text {
        text: "prompt".into(),
        images: vec![],
    });
    support::run_io(tokio::task::LocalSet::new().run_until(async {
        let running = tokio::task::spawn_local(tasks.next().await.unwrap());
        let provider = tasks.next().await.unwrap();
        provider.await;
        running.await.unwrap();
        observe.await.unwrap();
    }));
    assert_eq!(observations.get(), 1);
    let state = agent.state().read().unwrap();
    let history = state.messages().read().unwrap();
    let maestro_agent::AgentMessage::Assistant(message) = history.last().unwrap() else {
        panic!("missing assistant")
    };
    assert!(
        matches!(&message.read().unwrap().content[0], AssistantContent::Text(text) if text.text == "11")
    );
    registration.unregister();
}

/// Ordered hook and factory steps shared by one registered request.
type Trace = Arc<Mutex<Vec<&'static str>>>;
/// Factory consuming the payload selected by the original invocation hook.
#[derive(Default)]
struct SelectedPayload(Trace);
impl SelectedPayload {
    /// Execute against the original family's options after response observation.
    async fn produce<E: maestro_models::ProviderExecution>(
        trace: Trace,
        input: maestro_models::FauxResponseInput<E>,
    ) -> Result<maestro_models::AssistantMessage, maestro_models::DiagnosticErrorInfo> {
        trace.lock().unwrap().push("factory");
        let common = match input.options.unwrap() {
            maestro_models::StreamOptionsInput::Raw(options) => options.common,
            maestro_models::StreamOptionsInput::Simple(options) => options.common,
        };
        let selected = common.on_payload.unwrap()(
            maestro_models::PayloadHookInput {
                payload: serde_json::json!({"text": "unselected"}),
                signal: common.signal,
            },
            Arc::new(input.model),
        )
        .await?;
        Ok(
            support::assistant(selected.payload["text"].as_str().unwrap())
                .read()
                .unwrap()
                .clone(),
        )
    }
}
/// Concrete future erasure around the single generic factory body.
macro_rules! factory {
    ($mode:ty) => {
        impl maestro_models::Invoke<$mode, maestro_models::FauxResponseInput<$mode>>
            for SelectedPayload
        {
            type Return = <$mode as maestro_models::Execution>::Future<
                'static,
                Result<maestro_models::AssistantMessage, maestro_models::DiagnosticErrorInfo>,
            >;
            fn invoke(&self, input: maestro_models::FauxResponseInput<$mode>) -> Self::Return {
                Box::pin(Self::produce(Arc::clone(&self.0), input))
            }
        }
    };
}
factory!(Local);
factory!(maestro_models::Threaded);

/// A local payload selection that retains its capture across suspension.
fn payload_hook(text: &'static str) -> maestro_models::OnPayload<Local> {
    let selected = Rc::new(text);
    Rc::new(move |mut input, _| {
        let selected = Rc::clone(&selected);
        Box::pin(async move {
            tokio::task::yield_now().await;
            input.payload["text"] = serde_json::json!(*selected);
            Ok(input)
        })
    })
}

/// Local work queued through the existing event-stream owner.
type TaskQueue =
    maestro_models::EventStream<<Local as maestro_models::Execution>::Future<'static, ()>, ()>;
/// Retain task admission independently of result observation.
fn local_tasks() -> (TaskQueue, maestro_agent::AgentSpawner<Local>) {
    let tasks = TaskQueue::new_local_items(|_| false, |_| ());
    let queued = tasks.clone();
    (tasks, Rc::new(move |task| queued.push(task)))
}

#[test]
fn local_awaited_registered_dispatch_uses_required_submission() {
    let registration = registration("local-agent-awaited", 2);
    support::run_io(tokio::task::LocalSet::new().run_until(async {
        for continuation in [false, true] {
            let (tasks, spawn) = local_tasks();
            let observed = Rc::new(Cell::new(0));
            let mut config = local_config(registration.get_model(None).unwrap().clone());
            config.options.common.on_payload = Some(payload_hook("22"));
            config.options.common.on_response = Some(response_hook(Rc::clone(&observed)));
            let run = tokio::task::spawn_local(async move {
                awaited(
                    continuation,
                    &config,
                    maestro_agent::agent_loop::LocalAgentLoopOptions::RegisteredDispatch {
                        signal: None,
                        spawn,
                    },
                )
                .await
            });
            let provider = tasks.next().await.unwrap();
            assert_eq!(observed.get(), 0);
            provider.await;
            let messages = run.await.unwrap();
            assert_text(&messages, "22");
            assert_eq!(observed.get(), 1);
            tasks.end(Some(()));
        }
    }));
    assert_eq!(registration.state.call_count(), 2);
    registration.unregister();
}

/// Register isolated requests using one retained generic factory.
fn registration(api: &str, requests: usize) -> maestro_models::FauxProviderRegistration {
    let registration = maestro_models::register_faux_provider(RegisterFauxProviderOptions {
        api: Some(api.into()),
        ..Default::default()
    });
    let factory = Arc::new(SelectedPayload::default());
    registration.set_responses(
        (0..requests)
            .map(|_| FauxResponseStep::Factory(factory.clone()))
            .collect(),
    );
    registration
}

/// Capture a local response observer across a genuine suspension.
fn response_hook(observed: Rc<Cell<usize>>) -> maestro_models::OnResponse<Local> {
    Rc::new(move |_, _| {
        let observed = Rc::clone(&observed);
        Box::pin(async move {
            tokio::task::yield_now().await;
            observed.set(observed.get() + 1);
            Ok(())
        })
    })
}

/// Construct per-run local settings without introducing an alternate dispatch path.
fn local_config(model: maestro_models::Model) -> maestro_agent::LocalAgentLoopConfig {
    use maestro_agent::AgentExecution;
    maestro_agent::LocalAgentLoopConfig {
        model: Arc::new(std::sync::RwLock::new(model)),
        thinking_budgets: None,
        options: maestro_models::SimpleStreamOptions::default(),
        extra: maestro_models::JsonObject::new(),
        objects: maestro_models::ProviderObjects::default(),
        convert_to_llm: Local::default_converter(),
        transform_context: None,
        get_api_key: None,
        before_tool_call: None,
        after_tool_call: None,
        should_stop_after_turn: None,
        prepare_next_turn: None,
        get_steering_messages: None,
        get_follow_up_messages: None,
        tool_execution: None,
    }
}

/// Execute prompt and continuation through their public awaited entries.
async fn awaited(
    continuation: bool,
    config: &maestro_agent::LocalAgentLoopConfig,
    options: maestro_agent::agent_loop::LocalAgentLoopOptions,
) -> Vec<maestro_agent::AgentMessage> {
    let context = maestro_agent::LocalAgentContext {
        system_prompt: "instructions".into(),
        messages: Arc::new(std::sync::RwLock::new(vec![support::user("history")])),
        tools: None,
    };
    let emit = Rc::new(|_| {
        Box::pin(std::future::ready(Ok(())))
            as <Local as maestro_models::Execution>::Future<'static, _>
    });
    if continuation {
        maestro_agent::agent_loop::local::run_agent_loop_continue(context, config, emit, options)
            .await
            .unwrap()
    } else {
        maestro_agent::agent_loop::local::run_agent_loop(
            vec![support::user("prompt")],
            context,
            config,
            emit,
            options,
        )
        .await
        .unwrap()
    }
}

/// Observe the selected terminal body, not a callback's side channel.
fn assert_text(messages: &[maestro_agent::AgentMessage], expected: &str) {
    assert_eq!(assistant_text(&last_assistant(messages)), expected);
}
/// The retained terminal assistant entry.
fn last_assistant(messages: &[maestro_agent::AgentMessage]) -> maestro_models::AssistantMessage {
    let maestro_agent::AgentMessage::Assistant(message) = messages.last().unwrap() else {
        panic!("missing assistant")
    };
    message.read().unwrap().clone()
}
/// Text blocks of an assistant entry, in order.
fn assistant_text(message: &maestro_models::AssistantMessage) -> String {
    message
        .content
        .iter()
        .filter_map(|block| match block {
            AssistantContent::Text(text) => Some(text.text.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn local_replacement_stream_needs_no_provider_submission() {
    support::run_io(async {
        for continuation in [false, true] {
            let transformed = Rc::new(Cell::new(0));
            let seen = Rc::clone(&transformed);
            let mut config = local_config(support::model());
            config.options.common.on_payload = Some(payload_hook("replacement"));
            config.transform_context = Some(Rc::new(move |history, _| {
                seen.set(seen.get() + 1);
                history.write().unwrap().push(support::user("transformed"));
                Box::pin(std::future::ready(Ok(history)))
            }));
            let options = maestro_agent::agent_loop::LocalAgentLoopOptions::ReplacementStream {
                signal: None,
                stream_fn: replacement_stream(if continuation { 2 } else { 3 }),
            };
            let messages = awaited(continuation, &config, options).await;
            assert_text(&messages, "replacement");
            assert_eq!(transformed.get(), 1);
        }
    });
}

/// Replacement observes transformed messages and the original family hook handles.
fn replacement_stream(messages: usize) -> maestro_agent::StreamFn<Local> {
    Rc::new(move |model, context, options, open| {
        assert_eq!(context.messages.len(), messages);
        let simple_hook = options.common.on_payload.unwrap();
        assert!(Rc::ptr_eq(
            &simple_hook,
            open.common.on_payload.as_ref().unwrap()
        ));
        Box::pin(async move {
            let selected = simple_hook(
                maestro_models::PayloadHookInput {
                    payload: serde_json::json!({"text": "unselected"}),
                    signal: None,
                },
                Arc::new(model),
            )
            .await?;
            let stream = maestro_models::AssistantMessageEventStream::new();
            stream.end(Some(support::assistant(
                selected.payload["text"].as_str().unwrap(),
            )));
            Ok(stream)
        })
    })
}

#[test]
fn local_submitted_loops_reuse_their_task_owner() {
    let registration = registration("local-agent-submitted", 2);
    support::run_io(tokio::task::LocalSet::new().run_until(async {
        for continuation in [false, true] {
            let (tasks, spawn) = local_tasks();
            let observed = Rc::new(Cell::new(0));
            let mut config = local_config(registration.get_model(None).unwrap().clone());
            config.options.common.on_payload = Some(payload_hook("submitted"));
            config.options.common.on_response = Some(response_hook(Rc::clone(&observed)));
            let messages = Arc::new(std::sync::RwLock::new(vec![support::user("history")]));
            let context = maestro_agent::LocalAgentContext {
                system_prompt: String::new(),
                messages: Arc::clone(&messages),
                tools: None,
            };
            let stream = if continuation {
                maestro_agent::agent_loop::local::agent_loop_continue(
                    context,
                    Rc::new(config),
                    maestro_agent::agent_loop::AgentLoopOptionsRecord::default(),
                    spawn,
                )
                .unwrap()
            } else {
                maestro_agent::agent_loop::local::agent_loop(
                    vec![support::user("prompt")],
                    context,
                    Rc::new(config),
                    maestro_agent::agent_loop::AgentLoopOptionsRecord::default(),
                    spawn,
                )
            };
            let retained = (!continuation).then(|| stream.clone());
            drop(stream);
            let run = tokio::task::spawn_local(tasks.next().await.unwrap());
            let provider = tasks.next().await.unwrap();
            assert_eq!(observed.get(), 0);
            provider.await;
            run.await.unwrap();
            assert_eq!(observed.get(), 1);
            match retained {
                Some(stream) => assert_text(&stream.result().await, "submitted"),
                None => assert_text(&messages.read().unwrap(), "submitted"),
            }
            tasks.end(Some(()));
            assert!(tasks.next().await.is_none());
        }
    }));
    assert_eq!(registration.state.call_count(), 2);
    registration.unregister();
}

/// Recorded selections, owners and counters of the replacement sequence.
#[derive(Debug, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Replacement {
    /// Terminal text after each operation.
    outputs: Vec<String>,
    /// Owner of each response observation.
    response_owners: Vec<String>,
    /// Owner of each payload hook the provider invoked.
    captures: Vec<String>,
    /// Requests the provider received.
    call_count: usize,
    /// Responses still queued.
    pending: usize,
    /// Whether the agent is still streaming.
    streaming: bool,
}

#[test]
fn local_agent_replaced_hooks_are_captured_at_admission() {
    let scenario = scenarios()
        .into_iter()
        .find(|scenario| scenario.input.outcome == "replace_after_capture")
        .unwrap();
    assert_eq!(scenario.input.operation, "prompt_prompt_continue");
    let expected: Replacement = serde_json::from_value(scenario.expected).unwrap();
    let registration = registration("local-agent-live-hooks", 3);
    let (tasks, spawn) = local_tasks();
    let agent = Rc::new(Agent::<NoCustomMessages, Local>::new_local(AgentOptions {
        spawn: Some(spawn),
        ..Default::default()
    }));
    *agent.state().write().unwrap().model.write().unwrap() =
        registration.get_model(None).unwrap().clone();
    let owners = Rc::new(Owners::default());
    let old = labelled_hooks("11", "old", &owners);
    let old_captures = (Rc::downgrade(&old.0), Rc::downgrade(&old.1));
    install(&agent, Some(old));
    let entered = Rc::new(tokio::sync::Notify::new());
    let release = Rc::new(tokio::sync::Notify::new());
    *agent.get_api_key.write().unwrap() = Some(key_gate(Rc::clone(&entered), Rc::clone(&release)));
    let observe = agent.prompt(local_input());
    let mut outputs = Vec::new();
    support::run_io(tokio::task::LocalSet::new().run_until(async {
        let run = tokio::task::spawn_local(tasks.next().await.unwrap());
        entered.notified().await;
        let new = labelled_hooks("22", "new", &owners);
        let new_captures = (Rc::downgrade(&new.0), Rc::downgrade(&new.1));
        install(&agent, Some(new));
        assert_eq!(upgradable(&old_captures), (true, true));
        release.notify_one();
        tasks.next().await.unwrap().await;
        run.await.unwrap();
        observe.await.unwrap();
        assert_eq!(upgradable(&old_captures), (false, false));
        assert_eq!(owners.observed(), (vec!["old".into()], vec!["old".into()]));
        outputs.push(agent_text(&agent));
        *agent.get_api_key.write().unwrap() = None;
        drive_owned(&tasks, agent.prompt(local_input())).await;
        outputs.push(agent_text(&agent));
        agent.steer(support::user("steer"));
        drive_owned(&tasks, agent.r#continue()).await;
        outputs.push(agent_text(&agent));
        assert_eq!(upgradable(&new_captures), (true, true));
        install(&agent, None);
        assert_eq!(upgradable(&new_captures), (false, false));
        tasks.end(Some(()));
        assert!(tasks.next().await.is_none());
    }));
    let (response_owners, captures) = owners.observed();
    let observed = Replacement {
        outputs,
        response_owners,
        captures,
        call_count: registration.state.call_count(),
        pending: registration.get_pending_response_count(),
        streaming: agent.state().read().unwrap().is_streaming(),
    };
    assert_eq!(observed, expected);
    registration.unregister();
}

/// Owner labels of the hooks each operation actually invoked.
#[derive(Default)]
struct Owners {
    /// Response observer owners, in invocation order.
    responses: RefCell<Vec<String>>,
    /// Payload hook owners, in invocation order.
    captures: RefCell<Vec<String>>,
}
impl Owners {
    /// Copy both owner sequences.
    fn observed(&self) -> (Vec<String>, Vec<String>) {
        (
            self.responses.borrow().clone(),
            self.captures.borrow().clone(),
        )
    }
}

/// A local hook pair recording its owner after suspension; the payload hook selects `text`.
fn labelled_hooks(
    text: &'static str,
    owner: &'static str,
    owners: &Rc<Owners>,
) -> (
    maestro_models::OnPayload<Local>,
    maestro_models::OnResponse<Local>,
) {
    let (payload, response) = (Rc::clone(owners), Rc::clone(owners));
    let on_payload: maestro_models::OnPayload<Local> = Rc::new(move |mut input, _| {
        let owners = Rc::clone(&payload);
        Box::pin(async move {
            tokio::task::yield_now().await;
            owners.captures.borrow_mut().push(owner.into());
            input.payload["text"] = serde_json::json!(text);
            Ok(input)
        })
    });
    let on_response: maestro_models::OnResponse<Local> = Rc::new(move |_, _| {
        let owners = Rc::clone(&response);
        Box::pin(async move {
            tokio::task::yield_now().await;
            owners.responses.borrow_mut().push(owner.into());
            Ok(())
        })
    });
    (on_payload, on_response)
}

/// Move a hook pair into, or clear, the agent's live slots.
fn install(
    agent: &Agent<NoCustomMessages, Local>,
    hooks: Option<(
        maestro_models::OnPayload<Local>,
        maestro_models::OnResponse<Local>,
    )>,
) {
    let (payload, response) = hooks.unzip();
    *agent.on_payload.write().unwrap() = payload;
    *agent.on_response.write().unwrap() = response;
}

/// Whether any owner still retains each hook of a pair.
fn upgradable<P: ?Sized, R: ?Sized>(captures: &(Weak<P>, Weak<R>)) -> (bool, bool) {
    (captures.0.strong_count() > 0, captures.1.strong_count() > 0)
}

/// Suspend after the run captured its live hooks but before provider invocation.
fn key_gate(
    entered: Rc<tokio::sync::Notify>,
    release: Rc<tokio::sync::Notify>,
) -> <Local as maestro_models::Execution>::Shared<<Local as maestro_agent::AgentExecution>::Key> {
    Rc::new(move |_| {
        let entered = Rc::clone(&entered);
        let release = Rc::clone(&release);
        Box::pin(async move {
            entered.notify_one();
            release.notified().await;
            Ok(None)
        })
    })
}

/// Join independently admitted operation and provider work before observing completion.
async fn drive_owned(
    tasks: &TaskQueue,
    observe: maestro_models::BoxFuture<Result<(), maestro_models::DiagnosticErrorInfo>>,
) {
    let run = tokio::task::spawn_local(tasks.next().await.unwrap());
    tasks.next().await.unwrap().await;
    run.await.unwrap();
    observe.await.unwrap();
}

/// A default custom-message prompt in the local representation.
fn local_input() -> PromptInput {
    PromptInput::Text {
        text: "prompt".into(),
        images: vec![],
    }
}

/// Terminal text of the same agent history that model reduction changed.
fn agent_text(agent: &Agent<NoCustomMessages, Local>) -> String {
    let state = agent.state().read().unwrap();
    let messages = state.messages().read().unwrap();
    assistant_text(&last_assistant(&messages))
}
