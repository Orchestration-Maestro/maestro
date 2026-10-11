//! Prompt normalization and independently submitted execution.
use super::{Agent, AgentMessage, CustomAgentMessages, Native, lock, read, write};
use crate::{AgentContext, AgentLoopConfig, AgentLoopOptions, run_agent_loop};
use maestro_models::{
    BoxFuture, Cancellation, DiagnosticErrorInfo, EventStream, ImageContent, Message,
    ModelThinkingLevel, ProviderObjects, SimpleStreamOptions, StreamOptions, TextContent,
    UserBlock, UserContent, UserMessage, timestamp_now,
};
use std::sync::{Arc, RwLock};
#[cfg(not(target_arch = "wasm32"))]
/// Submit work to a caller-owned execution facility; dropping observation does not cancel it.
pub type AgentSpawner = Arc<dyn Fn(BoxFuture<()>) + Send + Sync>;
#[cfg(target_arch = "wasm32")]
/// Submit work to a caller-owned execution facility; dropping observation does not cancel it.
pub type AgentSpawner = Arc<dyn Fn(BoxFuture<()>)>;
/// Typed prompt overloads retaining supplied message identities.
pub enum PromptInput<C: CustomAgentMessages = std::convert::Infallible> {
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
        } = self.admit(&mut active, prompts, spawn);
        drop(active);
        spawn(task);
        observe
    }
    /// Capture run inputs while the admission reservation is held.
    fn admit(
        self: &Arc<Self>,
        active: &mut Option<ActiveRun>,
        prompts: Vec<AgentMessage<C>>,
        spawn: AgentSpawner,
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
        let config = Self::loop_config(model, &reasoning);
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
            let outcome = run_agent_loop(prompts, context, &config, emit, options).await;
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
    /// Defaults and standard-record conversion for the admitted run.
    fn loop_config(
        model: Arc<RwLock<maestro_models::Model>>,
        reasoning: &ModelThinkingLevel,
    ) -> AgentLoopConfig<C> {
        let convert: crate::ConvertToLlm<C> = Arc::new(|history| {
            let messages = read(&history)
                .iter()
                .filter_map(|entry| match entry {
                    AgentMessage::User(value) => Some(Message::User(read(value).clone())),
                    AgentMessage::Assistant(value) => Some(Message::Assistant(read(value).clone())),
                    AgentMessage::ToolResult(value) => {
                        Some(Message::ToolResult(read(value).clone()))
                    }
                    AgentMessage::Custom(_) => None,
                })
                .collect();
            Box::pin(std::future::ready(Ok(messages)))
        });
        AgentLoopConfig {
            model,
            convert_to_llm: convert,
            transform_context: None,
            get_api_key: None,
            before_tool_call: None,
            after_tool_call: None,
            should_stop_after_turn: None,
            get_steering_messages: None,
            get_follow_up_messages: None,
            tool_execution: Some(crate::ToolExecutionMode::Parallel),
            extra: [("toolExecution".into(), "parallel".into())]
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
                    transport: Some(maestro_models::Transport::Auto),
                    ..Default::default()
                },
                ..Default::default()
            },
        }
    }
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
