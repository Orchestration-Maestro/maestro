//! The session that frontends share: agent state, model and thinking selection, tool
//! selection, history navigation, statistics and exports.
mod export;
mod navigation;
mod selection;
mod skill_block;
mod state;
mod statistics;
mod tools;
mod types;

pub use skill_block::{ParsedSkillBlock, parse_skill_block};
pub use types::*;

use crate::resources::resource_loader::{LoadPromptsResult, ResourceLoader};
use crate::utils::lock::read;
use maestro_agent::{Agent, AgentState, QueueMode};
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
    /// Session listeners in subscription order.
    event_listeners: Rc<RefCell<Listeners<AgentSessionEventListener>>>,
    /// Scoped models and the tool registry.
    state: RefCell<AgentSessionState<E>>,
}

impl<E: ExtensionExecution> AgentSession<E> {
    /// Takes the supplied owners and the scoped models.
    #[must_use]
    pub fn new(config: AgentSessionConfig<E>) -> Self {
        Self {
            agent: config.agent,
            session_manager: config.session_manager,
            settings_manager: config.settings_manager,
            model_registry: config.model_registry,
            resource_loader: config.resource_loader,
            extension_runner: config.extension_runner,
            config_value_operations: config.config_value_operations,
            cwd: config.cwd,
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
        let identity = Rc::downgrade(&listener);
        let listeners = Rc::downgrade(&self.event_listeners);
        self.event_listeners.borrow_mut().add(listener);
        move || {
            if let Some(listeners) = listeners.upgrade() {
                let removed = listeners.borrow_mut().remove(identity.as_ptr());
                drop(removed);
            }
        }
    }

    /// Calls each listener in subscription order, including ones added meanwhile.
    fn emit(&self, event: &AgentSessionEvent) {
        let mut cursor = 0;
        loop {
            let next = self
                .event_listeners
                .borrow()
                .next_from(cursor)
                .map(|(next, listener)| (next, Rc::clone(listener)));
            let Some((next, listener)) = next else {
                break;
            };
            cursor = next;
            listener(event);
        }
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
}
