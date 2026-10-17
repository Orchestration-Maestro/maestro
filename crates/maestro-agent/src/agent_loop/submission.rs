//! Retained stream work submitted to the selected owner.
use super::{
    AgentContextRecord, AgentEvent, AgentEventSink, AgentExecution, AgentLoopConfigRecord,
    AgentLoopOptionsRecord, AgentMessage, AgentSpawner, CustomAgentMessages, DiagnosticErrorInfo,
    EventStream, check_continuation, prepare_prompt, prepared_loop,
};
use std::io::Write;
/// Concrete public entry signatures preserve the default generic arity.
macro_rules! entries {
    ($mode:ty, $awaited:ty) => {
        /// Submit owned prompt work.
        pub fn agent_loop<C: CustomAgentMessages + 'static>(
            prompts: Vec<AgentMessage<C>>,
            context: AgentContextRecord<C, $mode>,
            config: <$mode as maestro_models::Execution>::Shared<AgentLoopConfigRecord<C, $mode>>,
            options: AgentLoopOptionsRecord<$mode>,
            spawn: AgentSpawner<$mode>,
        ) -> EventStream<AgentEvent<C>, Vec<AgentMessage<C>>>
        where
            $mode: crate::ExecutionFor<C>,
        {
            submit(Some(prompts), context, config, options, spawn)
        }
        /// Submit continuation without replaying the existing tail.
        ///
        /// # Errors
        /// Returns an admission error for empty or assistant-ended history.
        pub fn agent_loop_continue<C: CustomAgentMessages + 'static>(
            context: AgentContextRecord<C, $mode>,
            config: <$mode as maestro_models::Execution>::Shared<AgentLoopConfigRecord<C, $mode>>,
            options: AgentLoopOptionsRecord<$mode>,
            spawn: AgentSpawner<$mode>,
        ) -> Result<EventStream<AgentEvent<C>, Vec<AgentMessage<C>>>, DiagnosticErrorInfo>
        where
            $mode: crate::ExecutionFor<C>,
        {
            check_continuation(&context)?;
            Ok(submit(None, context, config, options, spawn))
        }
        /// Await prompt work over a copied outer history.
        ///
        /// # Errors
        /// Returns a callback or conversion error.
        pub async fn run_agent_loop<C: CustomAgentMessages + 'static>(
            prompts: Vec<AgentMessage<C>>,
            context: AgentContextRecord<C, $mode>,
            config: &AgentLoopConfigRecord<C, $mode>,
            emit: AgentEventSink<C, $mode>,
            options: $awaited,
        ) -> Result<Vec<AgentMessage<C>>, DiagnosticErrorInfo>
        where
            $mode: crate::ExecutionFor<C>,
        {
            crate::agent_loop::run_for(Some(prompts), context, config, emit, options.into()).await
        }
        /// Await continuation through retained history.
        ///
        /// # Errors
        /// Returns an admission, callback or conversion error.
        pub async fn run_agent_loop_continue<C: CustomAgentMessages + 'static>(
            context: AgentContextRecord<C, $mode>,
            config: &AgentLoopConfigRecord<C, $mode>,
            emit: AgentEventSink<C, $mode>,
            options: $awaited,
        ) -> Result<Vec<AgentMessage<C>>, DiagnosticErrorInfo>
        where
            $mode: crate::ExecutionFor<C>,
        {
            crate::agent_loop::run_for(None, context, config, emit, options.into()).await
        }
    };
}
entries!(crate::Threaded, AgentLoopOptionsRecord);
/// Raw conversation entry points with local callables.
pub mod local {
    use super::{
        AgentContextRecord, AgentEvent, AgentEventSink, AgentLoopConfigRecord,
        AgentLoopOptionsRecord, AgentMessage, AgentSpawner, CustomAgentMessages,
        DiagnosticErrorInfo, EventStream, check_continuation, submit,
    };
    entries!(crate::Local, super::super::LocalAgentLoopOptions);
}
/// Construct the terminal-selected observation stream.
fn create_agent_stream<C: CustomAgentMessages + 'static>()
-> EventStream<AgentEvent<C>, Vec<AgentMessage<C>>> {
    EventStream::new_local_items(
        |event| matches!(event, AgentEvent::AgentEnd { .. }),
        |event| match event {
            AgentEvent::AgentEnd { messages } => messages.clone(),
            _ => Vec::new(),
        },
    )
}
/// Captured work for a submitted operation.
pub struct Submitted<C: CustomAgentMessages, E: AgentExecution> {
    /// Supplied prompts, absent for continuation.
    pub prompts: Vec<AgentMessage<C>>,
    /// Captured conversation handles.
    pub context: AgentContextRecord<C, E>,
    /// Captured callback and model configuration.
    pub config: E::Shared<AgentLoopConfigRecord<C, E>>,
    /// Captured invocation overrides.
    options: super::CapturedOptions<E>,
    /// Independent event and result observations.
    pub stream: EventStream<AgentEvent<C>, Vec<AgentMessage<C>>>,
    /// Callback admitting observations.
    pub emit: AgentEventSink<C, E>,
}
impl<C: CustomAgentMessages + 'static, E: crate::ExecutionFor<C>> Submitted<C, E> {
    /// Drive the captured operation to settlement.
    pub async fn drive(self) {
        let result = prepared_loop(
            self.prompts,
            self.context,
            &self.config,
            self.emit,
            self.options,
        )
        .await;
        match result {
            Ok(messages) => self.stream.end(Some(messages)),
            Err(error) => {
                let _ = writeln!(std::io::stderr(), "{}", error.message);
            }
        }
    }
}
/// Submit retained work once.
fn submit<C: CustomAgentMessages + 'static, E: crate::ExecutionFor<C>>(
    prompts: Option<Vec<AgentMessage<C>>>,
    mut context: AgentContextRecord<C, E>,
    config: E::Shared<AgentLoopConfigRecord<C, E>>,
    options: AgentLoopOptionsRecord<E>,
    spawn: AgentSpawner<E>,
) -> EventStream<AgentEvent<C>, Vec<AgentMessage<C>>> {
    if let Some(prompts) = &prompts {
        context = prepare_prompt(prompts, context);
    }
    let stream = create_agent_stream();
    let work = Submitted {
        prompts: prompts.unwrap_or_default(),
        context,
        config,
        options: super::CapturedOptions::new(options, E::provider_submission(spawn.clone())),
        stream: stream.clone(),
        emit: E::stream_sink(stream.clone()),
    };
    spawn(E::submitted(work));
    drop(spawn);
    stream
}
