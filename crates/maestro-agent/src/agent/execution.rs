//! Prompt and continuation admission with independently submitted execution.
use super::{Agent, AgentMessage, CustomAgentMessages, Native, lock, read, write};
use crate::{
    AgentContext, AgentLoopConfig, AgentLoopOptions, run_agent_loop, run_agent_loop_continue,
};
use maestro_models::{
    BoxFuture, Cancellation, DiagnosticErrorInfo, EventStream, ImageContent, ModelThinkingLevel,
    ProviderObjects, SimpleStreamOptions, StreamOptions, TextContent, UserBlock, UserContent,
    UserMessage, timestamp_now,
};
use std::sync::{Arc, RwLock};
#[cfg(not(target_arch = "wasm32"))]
/// Submit work to a caller-owned execution facility; dropping observation does not cancel it.
pub type AgentSpawner = Arc<dyn Fn(BoxFuture<()>) + Send + Sync>;
#[cfg(target_arch = "wasm32")]
/// Submit work to a caller-owned execution facility; dropping observation does not cancel it.
pub type AgentSpawner = Arc<dyn Fn(BoxFuture<()>)>;
/// Typed prompt overloads retaining supplied message identities.
pub enum PromptInput<C: CustomAgentMessages = crate::NoCustomMessages> {
    /// Text followed by ordered images.
    Text {
        /// Untrimmed text.
        text: String,
        /// Images in input order.
        images: Vec<ImageContent>,
    },
    /// A supplied entry.
    Message(AgentMessage<C>),
    /// An ordered batch, including an empty batch.
    Messages(Vec<AgentMessage<C>>),
}
/// Cancellation and completion retained during execution.
pub(super) struct ActiveRun {
    /// Fresh cooperative cancellation signal.
    pub signal: Cancellation,
    /// Completion without a transcript.
    pub result: EventStream<(), Result<(), DiagnosticErrorInfo>>,
}
/// Construct an authored operation diagnostic.
pub(super) fn failure(message: &str) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo {
        name: Some("Error".into()),
        message: message.into(),
        code: None,
        stack: None,
    }
}
impl<C: CustomAgentMessages + 'static> Agent<C> {
    #[cfg(not(target_arch = "wasm32"))]
    /// Reserve and submit a prompt synchronously; the returned future only observes.
    pub fn prompt(
        self: &Arc<Self>,
        input: PromptInput<C>,
    ) -> BoxFuture<Result<(), DiagnosticErrorInfo>>
    where
        C: Send + Sync,
    {
        self.start(input)
    }
    #[cfg(target_arch = "wasm32")]
    /// Reserve and submit a prompt synchronously; the returned future only observes.
    pub fn prompt(
        self: &Arc<Self>,
        input: PromptInput<C>,
    ) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        self.start(input)
    }
    #[cfg(not(target_arch = "wasm32"))]
    /// Reserve continuation without replaying history; queued assistant tails restart prompting.
    pub fn r#continue(self: &Arc<Self>) -> BoxFuture<Result<(), DiagnosticErrorInfo>>
    where
        C: Send + Sync,
    {
        self.continue_start()
    }
    #[cfg(target_arch = "wasm32")]
    /// Reserve continuation without replaying history; queued assistant tails restart prompting.
    pub fn r#continue(self: &Arc<Self>) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        self.continue_start()
    }
    /// Validate continuation under the shared reservation before consuming input.
    fn continue_start(self: &Arc<Self>) -> BoxFuture<Result<(), DiagnosticErrorInfo>>
    where
        C: Native,
    {
        let mut active = lock(&self.active);
        let error = if active.is_some() {
            Some("Agent is already processing. Wait for completion before continuing.")
        } else {
            let state = read(&self.state);
            match read(state.messages()).last() {
                None => Some("No messages to continue from"),
                Some(AgentMessage::Assistant(_)) if !self.has_queued_messages() => {
                    Some("Cannot continue from message role: assistant")
                }
                _ => None,
            }
        };
        if let Some(error) = error {
            return Box::pin(std::future::ready(Err(failure(error))));
        }
        let Some(spawn) = read(&self.spawn).clone() else {
            return Box::pin(std::future::ready(Err(failure(
                "Agent requires a task spawner before continuing.",
            ))));
        };
        let assistant = matches!(
            read(read(&self.state).messages()).last(),
            Some(AgentMessage::Assistant(_))
        );
        let (prompts, skip) = if assistant {
            let steering = self.drain_steering();
            if steering.is_empty() {
                (Some(self.drain_follow_up()), false)
            } else {
                (Some(steering), true)
            }
        } else {
            (None, false)
        };
        let Admitted {
            spawn,
            task,
            observe,
        } = self.admit(&mut active, prompts, spawn, skip);
        drop(active);
        spawn(task);
        observe
    }
    /// Reject busy runs before normalizing input.
    fn start(self: &Arc<Self>, input: PromptInput<C>) -> BoxFuture<Result<(), DiagnosticErrorInfo>>
    where
        C: Native,
    {
        let mut active = lock(&self.active);
        if active.is_some() {
            return Box::pin(std::future::ready(Err(failure(
                "Agent is already processing a prompt. Use steer() or followUp() to queue messages, or wait for completion.",
            ))));
        }
        let Some(spawn) = read(&self.spawn).clone() else {
            return Box::pin(std::future::ready(Err(failure(
                "Agent requires a task spawner before prompting.",
            ))));
        };
        let prompts = match input {
            PromptInput::Message(message) => vec![message],
            PromptInput::Messages(messages) => messages,
            PromptInput::Text { text, images } => {
                let mut content = vec![UserBlock::Text(TextContent {
                    text,
                    text_signature: None,
                })];
                content.extend(images.into_iter().map(UserBlock::Image));
                vec![AgentMessage::User(Arc::new(RwLock::new(UserMessage {
                    content: UserContent::Blocks(content),
                    timestamp: timestamp_now(),
                })))]
            }
        };
        let Admitted {
            spawn,
            task,
            observe,
        } = self.admit(&mut active, Some(prompts), spawn, false);
        drop(active);
        spawn(task);
        observe
    }
    /// Capture run inputs while the admission reservation is held.
    fn admit(
        self: &Arc<Self>,
        active: &mut Option<ActiveRun>,
        prompts: Option<Vec<AgentMessage<C>>>,
        spawn: AgentSpawner,
        skip: bool,
    ) -> Admitted
    where
        C: Native,
    {
        let signal = Cancellation::new();
        let result = EventStream::new(|(): &()| false, |(): &()| Ok(()));
        let observe = Box::pin(result.result());
        let mut state = write(&self.state);
        state.is_streaming = true;
        let retired = (state.streaming_message.take(), state.error_message.take());
        let context = AgentContext {
            system_prompt: state.system_prompt.clone(),
            messages: Arc::new(RwLock::new(read(state.messages()).clone())),
            tools: Some(Arc::new(RwLock::new(read(state.tools()).clone()))),
        };
        let model = Arc::clone(&state.model);
        let reasoning = state.thinking_level.clone();
        drop(state);
        let mut config = self.loop_config(model, &reasoning);
        self.install_queues(&mut config, skip);
        let stream_fn = read(&self.stream_fn).clone();
        *active = Some(ActiveRun {
            signal: signal.clone(),
            result: result.clone(),
        });
        let agent = Arc::clone(self);
        let emit_agent = Arc::clone(&agent);
        let emit_signal = signal.clone();
        let emit = Arc::new(move |event| {
            Box::pin(emit_agent.emit(event, emit_signal.clone())) as BoxFuture<_>
        });
        let task = Box::pin(async move {
            drop(retired);
            let options = AgentLoopOptions {
                signal: Some(signal.clone()),
                stream_fn,
            };
            let outcome = match prompts {
                Some(prompts) => run_agent_loop(prompts, context, &config, emit, options).await,
                None => run_agent_loop_continue(context, &config, emit, options).await,
            };
            let outcome = match outcome {
                Ok(_) => Ok(()),
                Err(error) => agent.handle_failure(error, signal).await,
            };
            agent.finish();
            drop(config);
            drop(agent);
            result.end(Some(outcome));
        });
        Admitted {
            spawn,
            task,
            observe,
        }
    }
    /// Capture live queue polls for only this task, with a one-shot steering skip.
    fn install_queues(self: &Arc<Self>, config: &mut AgentLoopConfig<C>, skip: bool)
    where
        C: Native,
    {
        let steering = Arc::clone(self);
        let skip = std::sync::atomic::AtomicBool::new(skip);
        config.get_steering_messages = Some(Arc::new(move || {
            let messages = if skip.swap(false, std::sync::atomic::Ordering::Relaxed) {
                Vec::new()
            } else {
                steering.drain_steering()
            };
            Box::pin(std::future::ready(Ok(messages)))
        }));
        let follow_up = Arc::clone(self);
        config.get_follow_up_messages = Some(Arc::new(move || {
            Box::pin(std::future::ready(Ok(follow_up.drain_follow_up())))
        }));
    }
    /// Capture request callbacks, scalar settings and retained object handles.
    fn loop_config(
        &self,
        model: Arc<RwLock<maestro_models::Model>>,
        reasoning: &ModelThinkingLevel,
    ) -> AgentLoopConfig<C> {
        let execution = *read(&self.tool_execution);
        AgentLoopConfig {
            model,
            thinking_budgets: read(&self.thinking_budgets).clone(),
            convert_to_llm: read(&self.convert_to_llm).clone(),
            transform_context: read(&self.transform_context).clone(),
            get_api_key: read(&self.get_api_key).clone(),
            before_tool_call: read(&self.before_tool_call).clone(),
            after_tool_call: read(&self.after_tool_call).clone(),
            should_stop_after_turn: None,
            get_steering_messages: None,
            get_follow_up_messages: None,
            tool_execution: Some(execution),
            extra: [(
                "toolExecution".into(),
                match execution {
                    crate::ToolExecutionMode::Parallel => "parallel".into(),
                    crate::ToolExecutionMode::Sequential => "sequential".into(),
                },
            )]
            .into_iter()
            .collect(),
            objects: ProviderObjects::default(),
            options: SimpleStreamOptions {
                reasoning: match reasoning {
                    ModelThinkingLevel::Off => None,
                    ModelThinkingLevel::Minimal => Some(maestro_models::ThinkingLevel::Minimal),
                    ModelThinkingLevel::Low => Some(maestro_models::ThinkingLevel::Low),
                    ModelThinkingLevel::Medium => Some(maestro_models::ThinkingLevel::Medium),
                    ModelThinkingLevel::High => Some(maestro_models::ThinkingLevel::High),
                    ModelThinkingLevel::Xhigh => Some(maestro_models::ThinkingLevel::Xhigh),
                },
                common: StreamOptions {
                    transport: Some(read(&self.transport).clone()),
                    session_id: read(&self.session_id).clone(),
                    max_retry_delay_ms: *read(&self.max_retry_delay_ms),
                    on_payload: read(&self.on_payload).clone(),
                    on_response: read(&self.on_response).clone(),
                    ..Default::default()
                },
                ..Default::default()
            },
        }
    }
}

/// Filter standard roles into owned model records at the request boundary.
pub(super) fn default_converter<C: CustomAgentMessages>() -> crate::ConvertToLlm<C> {
    Arc::new(|history| {
        let messages = read(&history)
            .iter()
            .filter_map(AgentMessage::to_message)
            .collect();
        Box::pin(std::future::ready(Ok(messages)))
    })
}

/// Submitted work and its independent observer.
struct Admitted {
    /// Captured task facility.
    spawn: AgentSpawner,
    /// Work retained by the task facility.
    task: BoxFuture<()>,
    /// Independent completion observation.
    observe: BoxFuture<Result<(), DiagnosticErrorInfo>>,
}
