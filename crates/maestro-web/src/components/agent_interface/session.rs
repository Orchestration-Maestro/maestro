//! Browser Agent binding, conditional request defaults and transcript composition.
use super::{empty::EmptySession, viewport::TranscriptViewport};
use crate::{
    browser::attachment_content::AttachmentHost,
    components::{
        message_list::{MessageList, MessageListProps},
        streaming_message_container::{StreamingMessageContainer, StreamingMessageProps},
    },
    tools::types::{ToolContentOutput, ToolRenderView, ToolUiError},
};
use maestro_app::browser::components::messages::CustomAgentMessages as BrowserMessages;
use maestro_app::{
    Agent, AgentEvent, AgentMessage, CustomAgentMessages, NoCustomMessages, Subscription,
};
use std::{cell::RefCell, collections::HashSet, rc::Rc};

/// Pending streaming replacement, distinguishing clear from no observation.
enum StreamObservation<C: CustomAgentMessages> {
    /// Preserve the current published/pending assistant.
    Unchanged,
    /// Retire the completed streaming subtree.
    Clear,
    /// Retire the finished run's streaming row before the Agent settles idle.
    End,
    /// Publish the latest assistant according to its reduced streaming state.
    Message(AgentMessage<BrowserMessages<C>>),
}
/// Latest reduced session observation, without a retained Agent guard.
struct Observation<C: CustomAgentMessages> {
    /// An admitted event requires fresh history/policy operands.
    refresh: bool,
    /// Latest streaming replacement; absent means unchanged.
    stream: StreamObservation<C>,
    /// Repaint owner learned on connection.
    context: Option<egui::Context>,
}
/// A retained view over the existing browser Agent, without editor or sending policy.
pub struct AgentInterface<C: CustomAgentMessages = NoCustomMessages> {
    /// Current usage activation, forwarded to stable and streaming assistants.
    pub on_cost_click: Option<Rc<dyn Fn()>>,
    /// Usage action acknowledged by the last successful native row publication.
    published_cost_action: Option<Rc<dyn Fn()>>,
    /// Retained external session owner.
    session: Option<Rc<Agent<BrowserMessages<C>>>>,
    /// Explicitly removed before replacing or releasing its session.
    subscription: Option<Subscription<BrowserMessages<C>>>,
    /// Weakly observed UI invalidation and latest streaming operands.
    observation: Rc<RefCell<Observation<C>>>,
    /// Stable positional widget lifetimes.
    list: MessageList<C>,
    /// Independently published assistant subtree.
    stream: StreamingMessageContainer<C>,
    /// One containing native scroll owner.
    viewport: TranscriptViewport,
    /// Native literal while no session is bound.
    empty: EmptySession,
    /// Available tool names from the latest admitted refresh.
    tools: Vec<String>,
    /// Pending calls from the latest admitted refresh.
    pending: HashSet<String>,
    /// Latest reduced streaming state.
    is_streaming: bool,
    /// Last drawn deferred publication.
    stream_revision: u64,
}
impl<C: CustomAgentMessages + 'static> AgentInterface<C> {
    /// Construct an unbound view using the existing native effect owner.
    #[must_use]
    pub fn new(host: Rc<dyn AttachmentHost>) -> Self {
        Self {
            on_cost_click: None,
            published_cost_action: None,
            session: None,
            subscription: None,
            observation: Rc::new(RefCell::new(Observation {
                refresh: true,
                stream: StreamObservation::Unchanged,
                context: None,
            })),
            list: MessageList::new(Rc::clone(&host)),
            stream: StreamingMessageContainer::new(Rc::clone(&host)),
            empty: EmptySession::new(Rc::clone(&host)),
            viewport: TranscriptViewport::new(host),
            tools: vec![],
            pending: HashSet::new(),
            is_streaming: false,
            stream_revision: 0,
        }
    }
    /// Return the retained active session, if bound.
    #[must_use]
    pub fn session(&self) -> Option<&Rc<Agent<BrowserMessages<C>>>> {
        self.session.as_ref()
    }
    /// Replace the binding, explicitly removing the previous observer first.
    /// # Errors
    /// Returns the original streaming publication failure.
    pub fn set_session(
        &mut self,
        session: Option<Rc<Agent<BrowserMessages<C>>>>,
    ) -> Result<(), ToolUiError> {
        if let Some(subscription) = self.subscription.take() {
            subscription.unsubscribe();
        }
        self.stream.set_message(None, true)?;
        self.session = session;
        {
            let mut observation = self.observation.borrow_mut();
            observation.refresh = true;
            observation.stream = StreamObservation::Unchanged;
        }
        if self.session.is_none() {
            self.list.refresh(&[])?;
            self.published_cost_action = None;
            self.viewport.clear();
            self.is_streaming = false;
        }
        if let Some(session) = &self.session {
            self.empty.clear();
            install_defaults(session);
            self.subscription = Some(subscribe(session, &self.observation));
        }
        if let Some(context) = &self.observation.borrow().context {
            context.request_repaint();
        }
        Ok(())
    }
    /// Override whether later native content growth follows the bottom.
    pub fn set_auto_scroll(&mut self, enabled: bool) {
        self.viewport.follow.enabled = enabled;
    }
    /// Render the native transcript or the literal unbound state.
    /// # Errors
    /// Returns reached child/host failures while transferring independently opened overlays.
    pub fn render(&mut self, ui: &mut egui::Ui, view: ToolRenderView<'_>) -> ToolContentOutput {
        let mut overlays = vec![];
        let (refresh, stream) = {
            let mut observation = self.observation.borrow_mut();
            observation.context = Some(ui.ctx().clone());
            (
                std::mem::take(&mut observation.refresh),
                std::mem::replace(&mut observation.stream, StreamObservation::Unchanged),
            )
        };
        let response = self.render_body(ui, view, &mut overlays, (refresh, &stream));
        if response.is_err() {
            let mut observation = self.observation.borrow_mut();
            observation.refresh = true;
            if matches!(observation.stream, StreamObservation::Unchanged) {
                observation.stream = stream;
            }
        }
        ToolContentOutput { response, overlays }
    }
    /// Refresh operands without retaining Agent/message guards across user callbacks.
    fn refresh(
        &mut self,
        (refresh, stream): (bool, &StreamObservation<C>),
    ) -> Result<bool, ToolUiError> {
        let ended = matches!(stream, StreamObservation::End);
        match stream {
            StreamObservation::Unchanged => {}
            StreamObservation::Clear | StreamObservation::End => {
                self.stream.set_message(None, true)?;
            }
            StreamObservation::Message(message) => {
                self.stream.set_message(Some(message.clone()), false)?;
            }
        }
        if refresh && let Some(session) = &self.session {
            let state = session
                .state()
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let messages = state
                .messages()
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            self.is_streaming = state.is_streaming() && !ended;
            self.pending = state.pending_tool_calls().iter().cloned().collect();
            self.tools = state
                .tools()
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .iter()
                .map(|tool| {
                    tool.read()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .definition
                        .name
                        .clone()
                })
                .collect();
            drop(state);
            self.list.refresh(&messages)?;
        }
        Ok(refresh)
    }
    /// Borrow row and policy owners only while drawing their admitted canvas positions.
    fn render_body(
        &mut self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
        overlays: &mut Vec<crate::AttachmentOverlay>,
        observation: (bool, &StreamObservation<C>),
    ) -> Result<egui::Response, ToolUiError> {
        if self.session.is_none() {
            return self.empty.render(ui, view);
        }
        let refresh = self.refresh(observation)? || self.cost_action_changed();
        let stable = self.list.len();
        let stream_revision = self.stream.revision();
        let dirty_stream =
            (stream_revision != self.stream_revision && self.is_streaming).then_some(stable);
        let props = MessageListProps {
            tools: &self.tools,
            pending_tool_calls: Some(&self.pending),
            is_streaming: self.is_streaming,
            on_cost_click: self.on_cost_click.clone(),
            ..Default::default()
        };
        let response = self.viewport.render(
            ui,
            (
                stable + usize::from(self.is_streaming),
                refresh,
                dirty_stream,
            ),
            view,
            |position, ui| {
                if position < stable {
                    self.list.draw_row(ui, position, &props, (view, overlays))
                } else {
                    self.stream
                        .render(
                            ui,
                            StreamingMessageProps {
                                tools: &self.tools,
                                is_streaming: self.is_streaming,
                                pending_tool_calls: Some(&self.pending),
                                tool_results_by_id: Some(self.list.results()),
                                on_cost_click: self.on_cost_click.clone(),
                            },
                            view,
                        )
                        .collect(overlays)
                        .map(|_| ())
                }
            },
        );
        if response.is_ok() {
            self.stream_revision = stream_revision;
            self.published_cost_action = self.on_cost_click.clone();
        }
        response
    }
    /// Changed callbacks must update attached native controls even outside the canvas viewport.
    fn cost_action_changed(&self) -> bool {
        match (&self.on_cost_click, &self.published_cost_action) {
            (Some(current), Some(published)) => !Rc::ptr_eq(current, published),
            (None, None) => false,
            _ => true,
        }
    }
}
impl<C: CustomAgentMessages> Drop for AgentInterface<C> {
    fn drop(&mut self) {
        if let Some(subscription) = self.subscription.take() {
            subscription.unsubscribe();
        }
    }
}
/// Capture session/UI backedges weakly; the Agent reducer has already run before this callback.
fn subscribe<C: CustomAgentMessages + 'static>(
    session: &Rc<Agent<BrowserMessages<C>>>,
    observation: &Rc<RefCell<Observation<C>>>,
) -> Subscription<BrowserMessages<C>> {
    let weak_session = Rc::downgrade(session);
    let weak_observation = Rc::downgrade(observation);
    session.subscribe(Rc::new(move |event, _| {
        if let (Some(_session), Some(observation)) =
            (weak_session.upgrade(), weak_observation.upgrade())
        {
            let event = event
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            observation.borrow_mut().apply(&event);
        }
        Box::pin(std::future::ready(Ok(())))
    }))
}
impl<C: CustomAgentMessages> Observation<C> {
    /// Admit transcript-producing events only, after the runtime reducer.
    fn apply(&mut self, event: &AgentEvent<BrowserMessages<C>>) {
        match event {
            AgentEvent::ToolExecutionStart { .. }
            | AgentEvent::ToolExecutionUpdate { .. }
            | AgentEvent::ToolExecutionEnd { .. } => return,
            AgentEvent::MessageEnd { .. } => self.stream = StreamObservation::Clear,
            AgentEvent::AgentEnd { .. } => self.stream = StreamObservation::End,
            AgentEvent::MessageUpdate { message, .. } => {
                self.stream = StreamObservation::Message(AgentMessage::Assistant(message.clone()));
            }
            _ => {}
        }
        self.refresh = true;
        if let Some(context) = &self.context {
            context.request_repaint();
        }
    }
}
/// Keep existing storage/request failure diagnostics intact at their consuming request boundary.
fn diagnostic(error: &std::io::Error) -> maestro_app::DiagnosticErrorInfo {
    maestro_app::DiagnosticErrorInfo::error(error.to_string())
}
/// Resolve storage separately after each await, retaining the typed URL read failure.
async fn proxy_url() -> Result<Option<String>, maestro_app::DiagnosticErrorInfo> {
    let enabled = crate::get_app_storage()
        .map_err(|error| diagnostic(&error))?
        .settings
        .get::<serde_json::Value>("proxy.enabled")
        .await
        .map_err(|error| diagnostic(&error))?;
    if !enabled.as_ref().is_some_and(maestro_app::is_truthy) {
        return Ok(None);
    }
    Ok(crate::get_app_storage()
        .map_err(|error| diagnostic(&error))?
        .settings
        .get::<String>("proxy.url")
        .await
        .map_err(|error| diagnostic(&error))?
        .filter(|url| !url.is_empty()))
}
/// Fill only absent operation slots and resolve the current application store at each reached read.
fn install_defaults<C: CustomAgentMessages>(session: &Agent<BrowserMessages<C>>) {
    let mut stream = session
        .stream_fn
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if stream.is_none() {
        *stream = Some(crate::create_stream_fn(Rc::new(|| Box::pin(proxy_url()))));
    }
    drop(stream);
    let mut key = session
        .get_api_key
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if key.is_none() {
        *key = Some(Rc::new(|provider| {
            Box::pin(async move {
                crate::get_app_storage()
                    .map_err(|error| diagnostic(&error))?
                    .provider_keys
                    .get(&provider)
                    .await
                    .map_err(|error| diagnostic(&error))
            })
        }));
    }
}
