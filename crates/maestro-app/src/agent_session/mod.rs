//! The session that frontends share: agent state, model and thinking selection, tool
//! selection, history navigation, statistics, exports, prompts, queues, direct shell
//! commands, the extension runtime and the agent-event dispatch.
mod bash;
mod compaction;
mod decisions;
mod events;
mod export;
mod extensions;
mod held;
mod lifecycle;
mod navigation;
mod prompting;
mod queues;
mod retry;
mod selection;
mod state;
mod statistics;
mod tool_runtime;
mod tools;
mod types;

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests;

pub use crate::skill_block::{ParsedSkillBlock, parse_skill_block};
pub use selection::delivery_mode;
pub(crate) use selection::{model_level, saved_level};
pub(crate) use state::{agent_messages, content_text, thrown};
pub use statistics::{UsageCostBreakdownEntry, UsageTotals, usage_cost_breakdown, usage_totals};
pub use types::*;

use crate::resources::resource_loader::{LoadPromptsResult, ResourceLoader};
use crate::utils::lock::{read, write};
use indexmap::{IndexMap, IndexSet};
use maestro_agent::Subscription;
use maestro_agent::{Agent, AgentListener, AgentSpawner, AgentState, QueueMode, SharedAgentTool};
use maestro_catalog::{ModelRegistry, ScopedModel};
use maestro_extensions::{
    ExtensionExecution, ExtensionRunner, ReplacedSessionContext, SessionStartEvent,
};
use maestro_listeners::Listeners;
use maestro_models::{DiagnosticErrorInfo, Local, Model};
use maestro_session::{
    Presence, SessionManager, messages::CustomAgentMessages,
    session_manager::ReadonlySessionManager,
};
use maestro_settings::SettingsManager;
use maestro_theme::LiveTheme;
use state::AgentSessionState;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// The session operations shared by every frontend, over the agent, the session history,
/// the settings, the model catalog and the loaded resources.
pub struct AgentSession<E: ExtensionExecution = Local> {
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
    /// The extensions' runner now in effect; every runtime build replaces it.
    extension_runner: RefCell<Rc<ExtensionRunner<E>>>,
    /// The caller's slot that mirrors the runner now in effect.
    extension_runner_ref: Option<ExtensionRunnerRef<E>>,
    /// The session working directory.
    cwd: String,
    /// The directory of the tool binaries that shell commands find first.
    bin_dir: String,
    /// The search index the built-in grep and find tools share.
    tools_manager: Arc<ToolsManager>,
    /// The theme extensions see while no user interface is bound.
    theme: LiveTheme,
    /// The only tool names exposed, when present.
    allowed_tool_names: Option<IndexSet<String>>,
    /// Executable tools used in place of the built-in ones.
    base_tools_override: Option<IndexMap<String, SharedAgentTool<Local>>>,
    /// The start event extensions receive when they are bound.
    session_start_event: SessionStartEvent,
    /// Session listeners in subscription order.
    event_listeners: Rc<RefCell<Listeners<AgentSessionEventListener>>>,
    /// The one agent listener, kept so a reconnection registers the same identity.
    agent_listener: AgentListener<CustomAgentMessages, Local>,
    /// The agent subscription, absent while disconnected.
    agent_subscription: RefCell<Option<Subscription<CustomAgentMessages, Local>>>,
    /// Scoped models, the tool registry and the session's running work.
    state: RefCell<AgentSessionState<E>>,
}

/// The task facility a session falls back on: `tokio::task::spawn_local`, so the work needs
/// the caller's Tokio `LocalSet`.
pub(crate) fn local_spawner() -> AgentSpawner<Local> {
    Rc::new(|task| drop(tokio::task::spawn_local(task)))
}

impl AgentSession<Local> {
    /// Takes the supplied owners, subscribes to the agent's events, installs the tool hooks
    /// and builds the extension runner, the tool registry and the base system prompt.
    /// Extensions receive the start event only once [`AgentSession::bind_extensions`] runs.
    ///
    /// Started work runs on the agent's own task facility. An agent without one gets
    /// `tokio::task::spawn_local`, so its work then needs the caller's Tokio `LocalSet`.
    ///
    /// # Errors
    /// Returns the documentation-path failure of the base prompt, or an extension error
    /// listener's failure while queued provider registrations are applied.
    pub fn new(config: AgentSessionConfig<Local>) -> Result<Rc<Self>, DiagnosticErrorInfo> {
        let active_tool_names = config.initial_active_tool_names.clone();
        let session = Rc::new_cyclic(|weak| Self::assemble(config, weak));
        session.finish_runtime_build(active_tool_names, true)?;
        Ok(session)
    }

    /// The session over `config`, subscribed to the agent, with its tool hooks and a fresh
    /// runner in place.
    fn assemble(config: AgentSessionConfig<Local>, weak: &std::rc::Weak<Self>) -> Self {
        let spawn = Rc::clone(write(&config.agent.spawn).get_or_insert_with(local_spawner));
        let agent_listener = events::agent_listener(weak);
        let subscription = config.agent.subscribe(Rc::clone(&agent_listener));
        tool_runtime::install_agent_tool_hooks(&config.agent, weak);
        let prompt_options = tools::prompt_options(
            &*config.resource_loader,
            &config.cwd,
            Vec::new(),
            IndexMap::new(),
            Vec::new(),
        );
        let runner = lifecycle::create_runner(
            &*config.resource_loader,
            &config.cwd,
            &config.session_manager,
            &config.model_registry,
            &config.theme,
        );
        if let Some(slot) = &config.extension_runner_ref {
            slot.borrow_mut().replace(Rc::clone(&runner));
        }
        Self {
            agent: config.agent,
            spawn,
            session_manager: config.session_manager,
            settings_manager: config.settings_manager,
            model_registry: config.model_registry,
            resource_loader: config.resource_loader,
            extension_runner: RefCell::new(runner),
            extension_runner_ref: config.extension_runner_ref,
            cwd: config.cwd,
            bin_dir: config.bin_dir,
            tools_manager: config.tools_manager,
            theme: config.theme,
            allowed_tool_names: config.allowed_tool_names.map(IndexSet::from_iter),
            base_tools_override: config.base_tools_override,
            session_start_event: config.session_start_event.unwrap_or(SessionStartEvent {
                reason: maestro_extensions::SessionStartReason::Startup,
                previous_session_file: Presence::Missing,
            }),
            event_listeners: Rc::default(),
            agent_listener,
            agent_subscription: RefCell::new(Some(subscription)),
            state: RefCell::new(AgentSessionState::new(
                config.scoped_models,
                config.custom_tools,
                prompt_options,
            )),
        }
    }
}

impl<E: ExtensionExecution> AgentSession<E> {
    /// The extensions' runner now in effect.
    #[must_use]
    pub fn extension_runner(&self) -> Rc<ExtensionRunner<E>> {
        Rc::clone(&self.extension_runner.borrow())
    }

    /// Whether any loaded extension handles `event_type`.
    #[must_use]
    pub fn has_extension_handlers(&self, event_type: &str) -> bool {
        self.extension_runner().has_handlers(event_type)
    }

    /// Stops delivering agent events to the session; the session listeners stay.
    pub(super) fn disconnect_from_agent(&self) {
        if let Some(subscription) = self.agent_subscription.borrow_mut().take() {
            subscription.unsubscribe();
        }
    }

    /// Delivers agent events to the session again after a disconnection.
    pub(super) fn reconnect_to_agent(&self) {
        let mut subscription = self.agent_subscription.borrow_mut();
        if subscription.is_none() {
            *subscription = Some(self.agent.subscribe(Rc::clone(&self.agent_listener)));
        }
    }

    /// Invalidates every extension context with the stale-context guidance, stops agent
    /// delivery, drops every session listener and releases the session's provider resources.
    /// Nothing running is aborted or awaited.
    pub fn dispose(&self) {
        self.extension_runner().invalidate(None);
        self.disconnect_from_agent();
        let listeners = std::mem::take(&mut *self.event_listeners.borrow_mut());
        drop(listeners);
        let session_id = self.session_id();
        drop(maestro_models::cleanup_session_resources(Some(&session_id)));
    }

    /// The runner's command context with this session's message operations, for work that
    /// continues against a replacement session.
    #[must_use]
    pub fn create_replaced_session_context(self: &Rc<Self>) -> ReplacedSessionContext
    where
        E: 'static,
    {
        let messages = Rc::clone(self);
        let users = Rc::clone(self);
        ReplacedSessionContext::new(
            self.extension_runner().create_command_context(),
            Rc::new(move |message, options| {
                let session = Rc::clone(&messages);
                Box::pin(async move {
                    session
                        .send_custom_message(message, options.unwrap_or_default())
                        .await
                        .map_err(Into::into)
                })
            }),
            Rc::new(move |content, options| {
                let session = Rc::clone(&users);
                Box::pin(async move {
                    session
                        .send_user_message(content, options.unwrap_or_default())
                        .await
                        .map_err(Into::into)
                })
            }),
        )
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

    /// The loader of the session's skills, prompt templates, extensions and context files.
    #[must_use]
    pub fn resource_loader(&self) -> &Rc<dyn ResourceLoader<E>> {
        &self.resource_loader
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
        let task = Box::pin(async move {
            drop(sender.send(operation.await));
        });
        crate::utils::task::start_now(task, |task| (self.spawn)(task));
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
