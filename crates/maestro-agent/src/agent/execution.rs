//! Prompt and continuation admission with independently submitted execution.
use super::{Agent, AgentMessage, CustomAgentMessages, lock, read, write};
use crate::agent_loop::AgentLoopOptionsRecord;
use crate::agent_loop::run_for;
use crate::types::{AgentContextRecord, AgentExecution, AgentLoopConfigRecord, Threaded};
use maestro_models::{
    BoxFuture, Cancellation, DiagnosticErrorInfo, EventStream, ImageContent, ModelThinkingLevel,
    ProviderObjects, SimpleStreamOptions, StreamOptions, TextContent, UserBlock, UserContent,
    UserMessage, timestamp_now,
};
use std::sync::{Arc, RwLock};
/// Caller-owned task submission.
pub type AgentSpawner<E = Threaded> =
    <E as maestro_models::Execution>::Shared<<E as AgentExecution>::Spawner>;
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
    DiagnosticErrorInfo::error(message)
}
/// Concrete receiver syntax delegates to shared admission.
macro_rules! entry {
    ($mode:ty,$ptr:ident) => {
        impl<C: CustomAgentMessages + 'static> Agent<C, $mode>
        where
            $mode: crate::ExecutionFor<C>,
        {
            /// Admit prompt work before returning its completion observer.
            pub fn prompt(
                self: &$ptr<Self>,
                input: PromptInput<C>,
            ) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
                Self::start(self, input)
            }
            /// Admit continuation before returning its completion observer.
            pub fn r#continue(self: &$ptr<Self>) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
                Self::continue_start(self)
            }
        }
    };
}
use std::rc::Rc;
entry!(crate::Local, Rc);
#[cfg(not(target_arch = "wasm32"))]
entry!(Threaded, Arc);
#[cfg(target_arch = "wasm32")]
entry!(Threaded, Rc);
impl<C: CustomAgentMessages + 'static, E: crate::ExecutionFor<C>> Agent<C, E> {
    /// Validate continuation under the shared reservation before consuming input.
    fn continue_start(agent: &E::Shared<Self>) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        let mut active = lock(&agent.active);
        let error = if active.is_some() {
            Some("Agent is already processing. Wait for completion before continuing.")
        } else {
            let state = read(&agent.state);
            match read(state.messages()).last() {
                None => Some("No messages to continue from"),
                Some(AgentMessage::Assistant(_)) if !agent.has_queued_messages() => {
                    Some("Cannot continue from message role: assistant")
                }
                _ => None,
            }
        };
        if let Some(error) = error {
            return Box::pin(std::future::ready(Err(failure(error))));
        }
        let Some(spawn) = read(&agent.spawn).clone() else {
            return Box::pin(std::future::ready(Err(failure(
                "Agent requires a task spawner before continuing.",
            ))));
        };
        let assistant = matches!(
            read(read(&agent.state).messages()).last(),
            Some(AgentMessage::Assistant(_))
        );
        let (prompts, skip) = if assistant {
            let steering = agent.drain_steering();
            if steering.is_empty() {
                (Some(agent.drain_follow_up()), false)
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
        } = Self::admit(agent, &mut active, prompts, spawn, skip);
        drop(active);
        spawn(task);
        observe
    }
    /// Reject busy runs before normalizing input.
    fn start(
        agent: &E::Shared<Self>,
        input: PromptInput<C>,
    ) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        let mut active = lock(&agent.active);
        if active.is_some() {
            return Box::pin(std::future::ready(Err(failure(
                "Agent is already processing a prompt. Use steer() or followUp() to queue messages, or wait for completion.",
            ))));
        }
        let Some(spawn) = read(&agent.spawn).clone() else {
            return Box::pin(std::future::ready(Err(failure(
                "Agent requires a task spawner before prompting.",
            ))));
        };
        let prompts = match input {
            PromptInput::Message(message) => vec![message],
            PromptInput::Messages(messages) => messages,
            PromptInput::Text { text, images } => {
                let mut content = vec![UserBlock::Text(TextContent::new(text))];
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
        } = Self::admit(agent, &mut active, Some(prompts), spawn, false);
        drop(active);
        spawn(task);
        observe
    }
    /// Capture run inputs while the admission reservation is held.
    fn admit(
        agent: &E::Shared<Self>,
        active: &mut Option<ActiveRun>,
        prompts: Option<Vec<AgentMessage<C>>>,
        spawn: AgentSpawner<E>,
        skip: bool,
    ) -> Admitted<E> {
        let signal = Cancellation::new();
        let result = EventStream::new(|(): &()| false, |(): &()| Ok(()));
        let observe = Box::pin(result.result());
        let mut state = write(&agent.state);
        state.is_streaming = true;
        let retired = (state.streaming_message.take(), state.error_message.take());
        let context = AgentContextRecord {
            system_prompt: state.system_prompt.clone(),
            messages: Arc::new(RwLock::new(read(state.messages()).clone())),
            tools: Some(E::share(RwLock::new(read(state.tools()).clone()))),
        };
        let model = Arc::clone(&state.model);
        let reasoning = state.thinking_level.clone();
        drop(state);
        let mut config = agent.loop_config(model, &reasoning);
        E::install_queues(agent.clone(), &mut config, skip);
        let stream_fn = read(&agent.stream_fn).clone();
        *active = Some(ActiveRun {
            signal: signal.clone(),
            result: result.clone(),
        });
        let task = E::owned(OwnedRun {
            agent: agent.clone(),
            prompts,
            context,
            config,
            options: crate::agent_loop::CapturedOptions::new(
                AgentLoopOptionsRecord {
                    signal: Some(signal.clone()),
                    stream_fn,
                },
                E::provider_submission(spawn.clone()),
            ),
            signal,
            result,
            retired,
        });
        Admitted {
            spawn,
            task,
            observe,
        }
    }
    /// Capture request callbacks, scalar settings and retained object handles.
    fn loop_config(
        &self,
        model: Arc<RwLock<maestro_models::Model>>,
        reasoning: &ModelThinkingLevel,
    ) -> AgentLoopConfigRecord<C, E> {
        let execution = *read(&self.tool_execution);
        AgentLoopConfigRecord {
            model,
            thinking_budgets: read(&self.thinking_budgets).clone(),
            convert_to_llm: read(&self.convert_to_llm).clone(),
            transform_context: read(&self.transform_context).clone(),
            get_api_key: read(&self.get_api_key).clone(),
            before_tool_call: read(&self.before_tool_call).clone(),
            after_tool_call: read(&self.after_tool_call).clone(),
            should_stop_after_turn: None,
            prepare_next_turn: read(&self.prepare_next_turn).clone(),
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
                reasoning: maestro_models::reasoning_from_thinking_level(reasoning),
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

/// Submitted lifecycle inputs, retaining the single shared algorithm.
pub struct OwnedRun<C: CustomAgentMessages, E: AgentExecution> {
    /// Owner retained for the admitted operation.
    pub agent: E::Shared<Agent<C, E>>,
    /// Supplied prompts, absent for continuation.
    pub prompts: Option<Vec<AgentMessage<C>>>,
    /// Captured conversation handles.
    pub context: AgentContextRecord<C, E>,
    /// Captured callback and model configuration.
    pub config: AgentLoopConfigRecord<C, E>,
    /// Captured invocation overrides and provider admission.
    options: crate::agent_loop::CapturedOptions<E>,
    /// Cancellation for the admitted operation.
    pub signal: Cancellation,
    /// Independent completion result.
    pub result: EventStream<(), Result<(), DiagnosticErrorInfo>>,
    /// Replaced values released outside admission locks.
    pub retired: (Option<AgentMessage<C>>, Option<String>),
}
impl<C: CustomAgentMessages + 'static, E: crate::ExecutionFor<C>> OwnedRun<C, E> {
    /// Drive the captured operation to settlement.
    pub async fn drive(self) {
        let emit = E::agent_sink(self.agent.clone(), self.signal.clone());
        drop(self.retired);
        let outcome = run_for(self.prompts, self.context, &self.config, emit, self.options).await;
        let outcome = match outcome {
            Ok(_) => Ok(()),
            Err(error) => {
                Agent::<C, E>::handle_failure(self.agent.clone(), error, self.signal).await
            }
        };
        self.agent.finish();
        drop(self.config);
        drop(self.agent);
        self.result.end(Some(outcome));
    }
}
/// Submitted work plus independent observation.
struct Admitted<E: AgentExecution> {
    /// Captured task admission callback.
    spawn: AgentSpawner<E>,
    /// Retained execution future.
    task: E::Future<'static, ()>,
    /// Independent completion observer.
    observe: BoxFuture<Result<(), DiagnosticErrorInfo>>,
}
