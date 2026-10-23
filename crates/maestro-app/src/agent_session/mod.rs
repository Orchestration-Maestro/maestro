//! The session that frontends share: agent state, model and thinking selection, tool
//! selection, history navigation, statistics, exports, prompts, queues and direct shell
//! commands.
mod bash;
mod compaction;
mod export;
mod navigation;
mod prompting;
mod queues;
mod retry;
mod selection;
mod skill_block;
mod state;
mod statistics;
mod tools;
mod types;

pub use skill_block::{ParsedSkillBlock, parse_skill_block};
pub use types::*;

use crate::resources::resource_loader::{LoadPromptsResult, ResourceLoader};
use crate::utils::lock::{read, write};
use maestro_agent::{Agent, AgentSpawner, AgentState, QueueMode};
use maestro_catalog::{ModelRegistry, ScopedModel};
use maestro_credentials::ConfigValueOperations;
use maestro_extensions::{ExtensionExecution, ExtensionRunner};
use maestro_models::{Local, Model, Threaded, records::listeners::Listeners};
use maestro_session::{
    SessionManager, messages::CustomAgentMessages, session_manager::ReadonlySessionManager,
};
use maestro_settings::SettingsManager;
use state::AgentSessionState;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// The session operations shared by every frontend, over the agent, the session history,
/// the settings, the model catalog and the loaded resources.
pub struct AgentSession<E: ExtensionExecution = Threaded> {
    /// The agent whose state the session reads and changes.
    agent: Rc<Agent<CustomAgentMessages, Local>>,
    /// The task facility that finishes the session's started work.
    spawn: AgentSpawner<Local>,
    /// The session history.
    session_manager: Rc<RefCell<SessionManager>>,
    /// The preferences.
    settings_manager: Rc<RefCell<SettingsManager>>,
    /// The model catalog.
    model_registry: Arc<ModelRegistry>,
    /// The loaded resources.
    resource_loader: Rc<dyn ResourceLoader<E>>,
    /// The extensions' runner.
    extension_runner: Rc<ExtensionRunner<E>>,
    /// Runs the commands that configured credentials name.
    config_value_operations: Arc<dyn ConfigValueOperations>,
    /// The session working directory.
    cwd: String,
    /// The directory of the tool binaries that shell commands find first.
    bin_dir: String,
    /// Session listeners in subscription order.
    event_listeners: Rc<RefCell<Listeners<AgentSessionEventListener>>>,
    /// Scoped models and the tool registry.
    state: RefCell<AgentSessionState<E>>,
}

impl<E: ExtensionExecution> AgentSession<E> {
    /// Takes the supplied owners and the scoped models.
    ///
    /// Started work runs on the agent's own task facility. An agent without one gets
    /// `tokio::task::spawn_local`, so its work then needs the caller's Tokio `LocalSet`.
    #[must_use]
    pub fn new(config: AgentSessionConfig<E>) -> Self {
        let spawn = Rc::clone(
            write(&config.agent.spawn)
                .get_or_insert_with(|| Rc::new(|task| drop(tokio::task::spawn_local(task)))),
        );
        Self {
            agent: config.agent,
            spawn,
            session_manager: config.session_manager,
            settings_manager: config.settings_manager,
            model_registry: config.model_registry,
            resource_loader: config.resource_loader,
            extension_runner: config.extension_runner,
            config_value_operations: config.config_value_operations,
            cwd: config.cwd,
            bin_dir: config.bin_dir,
            event_listeners: Rc::default(),
            state: RefCell::new(AgentSessionState::new(config.scoped_models)),
        }
    }

    /// The agent whose state the session reads and changes.
    #[must_use]
    pub fn agent(&self) -> &Rc<Agent<CustomAgentMessages, Local>> {
        &self.agent
    }

    /// The session history.
    #[must_use]
    pub fn session_manager(&self) -> &Rc<RefCell<SessionManager>> {
        &self.session_manager
    }

    /// The preferences.
    #[must_use]
    pub fn settings_manager(&self) -> &Rc<RefCell<SettingsManager>> {
        &self.settings_manager
    }

    /// The model catalog used for credentials and model discovery.
    #[must_use]
    pub fn model_registry(&self) -> &Arc<ModelRegistry> {
        &self.model_registry
    }

    /// Adds a listener; the returned operation removes it.
    pub fn subscribe(&self, listener: AgentSessionEventListener) -> impl Fn() + use<E> {
        Listeners::subscribe(&self.event_listeners, listener)
    }

    /// Calls each listener in subscription order, including ones added meanwhile.
    fn emit(&self, event: &AgentSessionEvent) {
        let Ok(()) = Listeners::try_for_each_live(&self.event_listeners, |listener| {
            listener(event);
            Ok::<(), std::convert::Infallible>(())
        });
    }

    /// The agent's live state.
    #[must_use]
    pub fn state(&self) -> &Rc<RwLock<AgentState<CustomAgentMessages, Local>>> {
        self.agent.state()
    }

    /// The selected model.
    #[must_use]
    pub fn model(&self) -> Arc<RwLock<Model>> {
        Arc::clone(&read(self.agent.state()).model)
    }

    /// The selected thinking level.
    #[must_use]
    pub fn thinking_level(&self) -> maestro_models::ModelThinkingLevel {
        read(self.agent.state()).thinking_level.clone()
    }

    /// Whether a response is streaming.
    #[must_use]
    pub fn is_streaming(&self) -> bool {
        read(self.agent.state()).is_streaming()
    }

    /// The current system prompt.
    #[must_use]
    pub fn system_prompt(&self) -> String {
        read(self.agent.state()).system_prompt.clone()
    }

    /// The live message history.
    #[must_use]
    pub fn messages(&self) -> Arc<RwLock<Vec<maestro_extensions::NativeAgentMessage>>> {
        Arc::clone(read(self.agent.state()).messages())
    }

    /// How steering messages are delivered.
    #[must_use]
    pub fn steering_mode(&self) -> QueueMode {
        self.agent.steering_mode()
    }

    /// How follow-up messages are delivered.
    #[must_use]
    pub fn follow_up_mode(&self) -> QueueMode {
        self.agent.follow_up_mode()
    }

    /// The session file, absent in memory.
    #[must_use]
    pub fn session_file(&self) -> Option<String> {
        self.session_manager
            .borrow()
            .get_session_file()
            .map(str::to_owned)
    }

    /// The session identity.
    #[must_use]
    pub fn session_id(&self) -> String {
        self.session_manager.borrow().get_session_id().to_owned()
    }

    /// The session display name.
    ///
    /// # Errors
    /// Returns invalid data when the newest name record holds neither text nor null.
    pub fn session_name(&self) -> std::io::Result<Option<String>> {
        Ok(self
            .session_manager
            .borrow()
            .get_session_name()?
            .map(str::to_owned))
    }

    /// The models cycled through, in order.
    #[must_use]
    pub fn scoped_models(&self) -> Vec<ScopedModel> {
        self.state.borrow().scoped_models.clone()
    }

    /// Replaces the models cycled through.
    pub fn set_scoped_models(&self, scoped_models: Vec<ScopedModel>) {
        self.state.borrow_mut().scoped_models = scoped_models;
    }

    /// The loaded prompt templates.
    #[must_use]
    pub fn prompt_templates(&self) -> Rc<LoadPromptsResult> {
        self.resource_loader.get_prompts()
    }

    /// Stops a retry, stops the running turn and waits for the agent to settle.
    pub async fn abort(&self) {
        self.abort_retry();
        self.agent.abort();
        self.agent.wait_for_idle().await;
    }

    /// Runs `operation` as the session's own work: polled once now, then finished on the
    /// session's task facility. The receiver only observes the result, so dropping it leaves
    /// the operation running.
    pub(super) fn start_operation<T: 'static>(
        &self,
        operation: impl std::future::Future<Output = T> + 'static,
    ) -> tokio::sync::oneshot::Receiver<T> {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let mut task: std::pin::Pin<Box<dyn std::future::Future<Output = ()>>> =
            Box::pin(async move {
                drop(sender.send(operation.await));
            });
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        if task.as_mut().poll(&mut context).is_pending() {
            (self.spawn)(task);
        }
        receiver
    }

    /// Runs one continuation after `delay` on the session's task facility.
    pub(super) fn schedule_continuation(&self, delay: std::time::Duration) {
        let due = tokio::time::Instant::now() + delay;
        let agent = Rc::clone(&self.agent);
        (self.spawn)(Box::pin(async move {
            // The wait is anchored now, so a late first poll does not delay the continuation.
            let remaining = due.saturating_duration_since(tokio::time::Instant::now());
            let _ = maestro_tools::process::sleep::sleep(remaining, None).await;
            drop(agent.r#continue().await);
        }));
    }

    /// Removes the newest agent message when it is a response `removable` accepts, keeping
    /// the stored history.
    pub(super) fn drop_trailing_assistant(
        &self,
        removable: impl Fn(&maestro_models::AssistantMessage) -> bool,
    ) {
        let messages = self.messages();
        let mut messages = write(&messages);
        let remove = matches!(
            messages.last(),
            Some(maestro_extensions::NativeAgentMessage::Assistant(last)) if removable(&read(last))
        );
        if remove {
            messages.pop();
        }
    }
}
