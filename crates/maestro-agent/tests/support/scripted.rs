//! Registered scripted-model fixtures and Agent history observations.
use maestro_agent::{
    Agent, AgentInitialState, AgentMessage, AgentOptions, AgentSpawner, ThinkingLevel,
};
use maestro_models::{
    AssistantContent, AssistantMessage, FauxAssistantMessageOptions, FauxProviderRegistration,
    FauxResponseStep, FauxToolCallOptions, Model, RegisterFauxProviderOptions, StopReason,
    TextContent, UserBlock, UserContent, UserMessage, faux_assistant_message, faux_text,
    faux_tool_call, register_faux_provider,
};
use serde_json::json;
use std::sync::{Arc, Mutex, RwLock};

/// Registrations removed when the scenario ends, even when it fails.
#[derive(Default)]
pub struct Registered(Vec<FauxProviderRegistration>);
impl Registered {
    /// Register a scripted provider and keep it until teardown.
    ///
    /// # Panics
    /// Panics if a controlled fixture lock is poisoned or a fixture is malformed.
    pub fn add(&mut self, options: RegisterFauxProviderOptions) -> &FauxProviderRegistration {
        self.0.push(register_faux_provider(options));
        self.0.last().unwrap()
    }
}
impl Drop for Registered {
    fn drop(&mut self) {
        for registration in &self.0 {
            registration.unregister();
        }
    }
}

/// A fixed scripted answer.
pub fn say(content: impl Into<maestro_models::FauxAssistantContent>) -> FauxResponseStep {
    let options = FauxAssistantMessageOptions::default();
    FauxResponseStep::Message(Box::new(faux_assistant_message(content, options)))
}
/// An answer that asks for the calculator.
///
/// # Panics
/// Panics if a controlled fixture lock is poisoned or a fixture is malformed.
#[must_use]
pub fn ask_calculator(id: &str, expression: &str) -> FauxResponseStep {
    let options = FauxToolCallOptions {
        id: Some(id.into()),
    };
    let content = vec![
        AssistantContent::Text(faux_text("Let me calculate that.")),
        AssistantContent::ToolCall(faux_tool_call(
            "calculate",
            json!({"expression": expression}),
            options,
        )),
    ];
    let options = FauxAssistantMessageOptions {
        stop_reason: Some(StopReason::ToolUse),
        ..Default::default()
    };
    FauxResponseStep::Message(Box::new(faux_assistant_message(content, options)))
}
/// Spawn submitted work on the running test runtime.
#[must_use]
pub fn spawner() -> AgentSpawner {
    Arc::new(|task| {
        tokio::spawn(task);
    })
}
/// An Agent over a registered model.
#[must_use]
pub fn agent_for(model: &Model, tools: Vec<maestro_agent::SharedAgentTool>) -> Arc<Agent> {
    Arc::new(Agent::new(AgentOptions {
        spawn: Some(spawner()),
        initial_state: AgentInitialState {
            system_prompt: Some("You are a helpful assistant.".into()),
            model: Some(Arc::new(RwLock::new(model.clone()))),
            thinking_level: Some(ThinkingLevel::Off),
            tools,
            messages: vec![],
        },
        ..Default::default()
    }))
}
/// A user entry made of one text block.
#[must_use]
pub fn user<C: maestro_agent::CustomAgentMessages>(text: &str) -> AgentMessage<C> {
    let block = UserBlock::Text(TextContent::new(text.into()));
    AgentMessage::User(Arc::new(RwLock::new(UserMessage {
        content: UserContent::Blocks(vec![block]),
        timestamp: 1.0,
    })))
}
/// Replace the conversation history.
///
/// # Panics
/// Panics if a controlled fixture lock is poisoned or a fixture is malformed.
pub fn set_history(agent: &Agent, messages: Vec<AgentMessage>) {
    agent.state().write().unwrap().set_messages(messages);
}
/// The text of a user block, if it is text.
#[must_use]
pub fn block_text(block: &UserBlock) -> Option<&str> {
    match block {
        UserBlock::Text(text) => Some(text.text.as_str()),
        UserBlock::Image(_) => None,
    }
}
/// The text blocks of one entry, with its role.
///
/// # Panics
/// Panics if a controlled fixture lock is poisoned or a fixture is malformed.
#[must_use]
pub fn describe(message: &AgentMessage) -> (&'static str, String) {
    match message {
        AgentMessage::User(user) => match &user.read().unwrap().content {
            UserContent::Text(text) => ("user", text.clone()),
            UserContent::Blocks(blocks) => ("user", UserBlock::text_output(blocks)),
        },
        AgentMessage::Assistant(assistant) => {
            let content = &assistant.read().unwrap().content;
            let texts = content.iter().filter_map(|block| match block {
                AssistantContent::Text(text) => Some(text.text.as_str()),
                _ => None,
            });
            ("assistant", texts.collect::<Vec<_>>().join("\n"))
        }
        AgentMessage::ToolResult(result) => (
            "toolResult",
            UserBlock::text_output(&result.read().unwrap().content),
        ),
        AgentMessage::Custom(never) => match *never.read().unwrap() {},
    }
}
/// Role and text of every history entry.
///
/// # Panics
/// Panics if a controlled fixture lock is poisoned or a fixture is malformed.
pub fn history(agent: &Agent) -> Vec<(&'static str, String)> {
    let state = agent.state().read().unwrap();
    let messages = state.messages().read().unwrap();
    messages.iter().map(describe).collect()
}
/// The last history entry, which must be an assistant response.
///
/// # Panics
/// Panics if a controlled fixture lock is poisoned or a fixture is malformed.
pub fn last_assistant(agent: &Agent) -> AssistantMessage {
    let state = agent.state().read().unwrap();
    let messages = state.messages().read().unwrap();
    let Some(AgentMessage::Assistant(message)) = messages.last() else {
        panic!("expected an assistant entry last");
    };
    message.read().unwrap().clone()
}
/// Whether the Agent is streaming.
///
/// # Panics
/// Panics if a controlled fixture lock is poisoned or a fixture is malformed.
pub fn streaming(agent: &Agent) -> bool {
    agent.state().read().unwrap().is_streaming()
}
/// Record the label of every event the Agent emits.
///
/// # Panics
/// Panics if a controlled fixture lock is poisoned or a fixture is malformed.
pub fn record_events(agent: &Arc<Agent>) -> Arc<Mutex<Vec<&'static str>>> {
    let events = Arc::new(Mutex::new(vec![]));
    let sink = Arc::clone(&events);
    agent.subscribe(Arc::new(move |event, _| {
        sink.lock()
            .unwrap()
            .push(super::observable::kind(&event.read().unwrap()));
        Box::pin(std::future::ready(Ok(())))
    }));
    events
}

/// Erase the two future representations beside a generic scripted response body.
macro_rules! factory {
    ($owner:ty, $erase:expr) => {
        $crate::support::scripted::factory!($owner, maestro_models::Local, $erase);
        $crate::support::scripted::factory!($owner, maestro_models::Threaded, $erase);
    };
    ($owner:ty, $mode:ty, $erase:expr) => {
        impl maestro_models::Invoke<$mode, maestro_models::FauxResponseInput<$mode>> for $owner {
            type Return = <$mode as maestro_models::Execution>::Future<
                'static,
                Result<maestro_models::AssistantMessage, maestro_models::DiagnosticErrorInfo>,
            >;
            fn invoke(&self, input: maestro_models::FauxResponseInput<$mode>) -> Self::Return {
                Box::pin(($erase)(self, input))
            }
        }
    };
}
pub(crate) use factory;
