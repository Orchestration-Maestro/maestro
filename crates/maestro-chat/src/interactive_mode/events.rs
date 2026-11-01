//! The single session-event subscriber, the metadata it refreshes and the streamed
//! assistant and tool rows it keeps.

use super::InteractiveMode;
use super::progress::{CompactionOutcome, RetryEnd, RetryStart};
use super::replay::{aborted_notice, error_notice, failure_result};
use crate::{AssistantMessageComponent, ToolExecutionComponent, ToolExecutionResult};
use maestro_app::configuration::config::APP_TITLE;
use maestro_app::messages::CustomAgentMessages;
use maestro_app::{
    AgentEvent, AgentMessage, AgentSessionEvent, AgentToolResult, AssistantContent,
    NativeAgentMessage, ReadonlySessionManager, SharedAssistantMessage, StopReason, ToolCall,
};
use maestro_tui::Component;
use std::{
    error::Error,
    io,
    rc::Rc,
    sync::Arc,
    sync::{PoisonError, RwLockReadGuard},
};

impl InteractiveMode {
    /// Registers the one session listener with a weak mode handle; its removal operation
    /// stays here for the session lifecycle.
    pub(super) fn subscribe_to_agent(self: &Rc<Self>) {
        let mode = Rc::downgrade(self);
        let removal =
            self.runtime_host
                .session()
                .subscribe(Rc::new(move |event: &AgentSessionEvent| {
                    let Some(mode) = mode.upgrade() else {
                        return;
                    };
                    if let Err(error) = mode.handle_event(event) {
                        mode.report(Err(error));
                    }
                }));
        self.unsubscribe.replace(Some(Box::new(removal)));
    }

    /// Handles one session event, initializing the presentation first, then refreshing the
    /// metadata the event names from the session's live state or following the agent.
    ///
    /// # Errors
    /// Returns a synchronous presentation failure. Deferred initialization and event
    /// failures go to the supplied runtime's error route.
    pub(super) fn handle_event(
        self: &Rc<Self>,
        event: &AgentSessionEvent,
    ) -> Result<(), Box<dyn Error>> {
        if self.is_initialized.get() && !self.event_initialization_pending.get() {
            return self.handle_initialized_event(event);
        }
        self.pending_events.borrow_mut().push_back(event.clone());
        if self.event_initialization_pending.replace(true) {
            return Ok(());
        }
        let mode = Rc::clone(self);
        self.tool_environment
            .runtime
            .spawn_local(Box::pin(async move {
                let result = mode.initialize_pending_events().await;
                mode.event_initialization_pending.set(false);
                mode.pending_events.borrow_mut().clear();
                result
            }));
        Ok(())
    }

    /// Initializes the presentation before delivering the events waiting for it.
    async fn initialize_pending_events(self: &Rc<Self>) -> Result<(), Box<dyn Error>> {
        self.init().await?;
        loop {
            let event = self.pending_events.borrow_mut().pop_front();
            let Some(event) = event else { return Ok(()) };
            self.report(self.handle_initialized_event(&event));
        }
    }

    /// Presents one event after initialization has completed.
    fn handle_initialized_event(
        self: &Rc<Self>,
        event: &AgentSessionEvent,
    ) -> Result<(), Box<dyn Error>> {
        self.footer.invalidate();
        match event {
            AgentSessionEvent::QueueUpdate { .. } => {
                self.update_pending_messages_display()?;
                self.ui.request_render(false);
            }
            AgentSessionEvent::SessionInfoChanged { .. } => {
                self.update_terminal_title()
                    .map_err(|error| -> Box<dyn Error> { error })?;
                self.ui.request_render(false);
            }
            AgentSessionEvent::ThinkingLevelChanged { .. } => self.update_editor_border_color()?,
            AgentSessionEvent::CompactionStart { reason } => self.start_compaction(*reason)?,
            AgentSessionEvent::CompactionEnd {
                reason,
                result,
                aborted,
                error_message,
                will_retry,
            } => self.end_compaction(&CompactionOutcome {
                reason: *reason,
                result: result.as_ref(),
                aborted: *aborted,
                error_message: error_message.as_deref(),
                will_retry: *will_retry,
            })?,
            AgentSessionEvent::AutoRetryStart {
                attempt,
                max_attempts,
                delay_ms,
                ..
            } => self.start_retry(&RetryStart {
                attempt: *attempt,
                max_attempts: *max_attempts,
                delay_ms: *delay_ms,
            }),
            AgentSessionEvent::AutoRetryEnd {
                success,
                attempt,
                final_error,
            } => self.end_retry(&RetryEnd {
                success: *success,
                attempt: *attempt,
                final_error: final_error.as_deref(),
            })?,
            AgentSessionEvent::Agent(event) => {
                let event = event.read().unwrap_or_else(PoisonError::into_inner);
                self.dispatch_agent_event(event)?;
            }
        }
        Ok(())
    }

    /// Routes a live agent event, leaving nonvisual boundaries without a frame.
    ///
    /// # Errors
    /// Returns the failure of the agent event's presentation.
    fn dispatch_agent_event(
        &self,
        event: RwLockReadGuard<'_, AgentEvent<CustomAgentMessages>>,
    ) -> Result<(), Box<dyn Error>> {
        match &*event {
            AgentEvent::MessageStart {
                message: AgentMessage::ToolResult(_),
            }
            | AgentEvent::MessageEnd {
                message: AgentMessage::User(_),
            } => {}
            AgentEvent::MessageEnd {
                message: AgentMessage::Custom(_) | AgentMessage::ToolResult(_),
            } => self.ui.request_render(false),
            _ => self.handle_agent_event(event)?,
        }
        Ok(())
    }

    /// Follows one agent event in the working row, the transcript and the pending calls.
    ///
    /// # Errors
    /// Returns the failure of the terminal progress, of the published theme or of a row's
    /// rendering.
    fn handle_agent_event(
        &self,
        event: RwLockReadGuard<'_, AgentEvent<CustomAgentMessages>>,
    ) -> Result<(), Box<dyn Error>> {
        // Copy only consumed operands, releasing the guard before any renderer runs.
        match &*event {
            AgentEvent::AgentStart => {
                drop(event);
                self.start_agent_progress();
                self.handle_agent_start()?;
            }
            AgentEvent::MessageStart { message } => {
                let message = message.clone();
                drop(event);
                self.handle_message_start(&message)?;
            }
            AgentEvent::MessageUpdate { message, .. } => {
                let message = Arc::clone(message);
                drop(event);
                self.handle_message_update(&message)?;
            }
            AgentEvent::MessageEnd { message } => {
                let message = message.clone();
                drop(event);
                self.handle_message_end(&message)?;
            }
            AgentEvent::ToolExecutionStart { tool_call } => {
                let row = self
                    .pending_tools
                    .borrow()
                    .get(&tool_call.id)
                    .cloned()
                    .ok_or_else(|| tool_call.clone());
                drop(event);
                self.handle_tool_execution_start(row)?;
            }
            AgentEvent::ToolExecutionUpdate {
                tool_call,
                partial_result,
            } => {
                let (id, result) = (tool_call.id.clone(), partial_result.clone());
                drop(event);
                self.show_tool_result(&id, result, None)?;
            }
            AgentEvent::ToolExecutionEnd {
                tool_call,
                result,
                is_error,
            } => {
                let (id, result, failed) = (tool_call.id.clone(), result.clone(), *is_error);
                drop(event);
                self.show_tool_result(&id, result, Some(failed))?;
            }
            AgentEvent::AgentEnd { .. } => {
                drop(event);
                self.handle_agent_end()?;
            }
            AgentEvent::TurnStart | AgentEvent::TurnEnd { .. } => {}
        }
        Ok(())
    }

    /// Forgets the pending calls, shows terminal progress when configured and replaces the
    /// working row, unless an extension hid it.
    ///
    /// # Errors
    /// Returns the terminal's progress-writing failure.
    fn handle_agent_start(&self) -> io::Result<()> {
        drop(self.pending_tools.take());
        self.set_terminal_progress(true)?;
        self.stop_working_loader();
        if self.working.working_visible.get() {
            self.show_working_loader();
        }
        self.ui.request_render(false);
        Ok(())
    }

    /// Adds a custom or user message to the transcript, the user one refreshing the pending
    /// rows, or starts the streaming row of an assistant message; a tool result adds nothing.
    ///
    /// # Errors
    /// Returns the failure of a row's rendering or of the published theme.
    fn handle_message_start(&self, message: &NativeAgentMessage) -> Result<(), Box<dyn Error>> {
        match message {
            AgentMessage::Custom(_) => self
                .add_message_to_chat(message, false)
                .map_err(|error| -> Box<dyn Error> { error })?,
            AgentMessage::User(_) => {
                self.add_message_to_chat(message, false)
                    .map_err(|error| -> Box<dyn Error> { error })?;
                self.update_pending_messages_display()?;
            }
            AgentMessage::Assistant(message) => {
                let component = self.assistant_component(Arc::clone(message));
                self.chat_container.add_child(component.clone());
                drop(
                    self.streaming
                        .replace(Some((component, Arc::clone(message)))),
                );
            }
            AgentMessage::ToolResult(_) => return Ok(()),
        }
        self.ui.request_render(false);
        Ok(())
    }

    /// Shows `message` in the streaming row and gives each of its tool calls a pending row,
    /// updating the arguments of a call that already has one; ignored without a stream.
    ///
    /// # Errors
    /// Returns the failure of a renderer binding or of a row's rendering.
    fn handle_message_update(
        &self,
        message: &SharedAssistantMessage,
    ) -> Result<(), Box<dyn Error>> {
        let Some(component) = self.streaming_component() else {
            return Ok(());
        };
        drop(
            self.streaming
                .replace(Some((Rc::clone(&component), Arc::clone(message)))),
        );
        component.update_content(Arc::clone(message));
        let calls: Vec<ToolCall> = {
            let message = message.read().unwrap_or_else(PoisonError::into_inner);
            let calls = message.content.iter().filter_map(|content| match content {
                AssistantContent::ToolCall(call) => Some(call.clone()),
                AssistantContent::Text(_) | AssistantContent::Thinking(_) => None,
            });
            calls.collect()
        };
        for call in calls {
            let row = self.pending_tools.borrow().get(&call.id).cloned();
            match row {
                Some(row) => row.update_args(Some(call.arguments))?,
                None => drop(self.add_pending_tool(call)?),
            }
        }
        self.ui.request_render(false);
        Ok(())
    }

    /// Completes the streaming row with the ended assistant `message`; a user message ends
    /// nothing and requests no frame.
    ///
    /// # Errors
    /// Returns the failure of a row's rendering.
    fn handle_message_end(&self, message: &NativeAgentMessage) -> Result<(), Box<dyn Error>> {
        if let AgentMessage::User(_) = message {
            return Ok(());
        }
        if let (Some(component), AgentMessage::Assistant(message)) =
            (self.streaming_component(), message)
        {
            self.complete_streaming(&component, message)?;
        }
        self.ui.request_render(false);
        Ok(())
    }

    /// Shows the ended `message` in `component`, writing an abort's notice into the message,
    /// then completes the pending calls and releases the stream.
    ///
    /// # Errors
    /// Returns the failure of a row's rendering.
    fn complete_streaming(
        &self,
        component: &AssistantMessageComponent,
        message: &SharedAssistantMessage,
    ) -> Result<(), Box<dyn Error>> {
        let attempts = self.runtime_host.session().retry_attempt();
        let failure = {
            let mut message = message.write().unwrap_or_else(PoisonError::into_inner);
            match message.stop_reason {
                StopReason::Aborted => {
                    let notice = aborted_notice(attempts);
                    message.error_message = Some(notice.clone());
                    Some(notice)
                }
                StopReason::Error => Some(error_notice(message.error_message.as_deref())),
                _ => None,
            }
        };
        component.update_content(Arc::clone(message));
        self.finish_pending_tools(failure.as_deref())?;
        drop(self.streaming.take());
        self.footer.invalidate();
        Ok(())
    }

    /// Completes every pending row with `failure` as its error and forgets them, or, without a
    /// failure, marks their arguments complete.
    ///
    /// # Errors
    /// Returns the failure of a row's rendering.
    fn finish_pending_tools(&self, failure: Option<&str>) -> Result<(), Box<dyn Error>> {
        let rows: Vec<_> = self.pending_tools.borrow().values().cloned().collect();
        for row in rows {
            match failure {
                Some(text) => row.update_result(failure_result(text), false)?,
                None => row.set_args_complete()?,
            }
        }
        if failure.is_some() {
            drop(self.pending_tools.take());
        }
        Ok(())
    }

    /// Marks an existing row started, creating a pending row for an unknown call.
    ///
    /// # Errors
    /// Returns the failure of a renderer binding or of the row's rendering.
    fn handle_tool_execution_start(
        &self,
        row: Result<Rc<ToolExecutionComponent>, ToolCall>,
    ) -> Result<(), Box<dyn Error>> {
        let row = match row {
            Ok(row) => row,
            Err(call) => self.add_pending_tool(call)?,
        };
        row.mark_execution_started()?;
        self.ui.request_render(false);
        Ok(())
    }

    /// Shows `result` in the pending row of call `id`: as progress without `failed`, else as
    /// the final result with that failure flag, forgetting the call. An unknown call shows
    /// nothing.
    ///
    /// # Errors
    /// Returns the failure of the row's rendering.
    fn show_tool_result(
        &self,
        id: &str,
        result: AgentToolResult,
        failed: Option<bool>,
    ) -> Result<(), Box<dyn Error>> {
        let Some(row) = self.pending_tools.borrow().get(id).cloned() else {
            return Ok(());
        };
        let result = ToolExecutionResult {
            content: result.content,
            details: result.details,
            is_error: failed.unwrap_or(false),
        };
        row.update_result(result, failed.is_none())?;
        if failed.is_some() {
            drop(self.pending_tools.borrow_mut().remove(id));
        }
        self.ui.request_render(false);
        Ok(())
    }

    /// Clears terminal progress when configured, stops the working row, removes the
    /// unfinished streaming row and forgets the pending calls.
    ///
    /// # Errors
    /// Returns the terminal's progress-writing failure.
    fn handle_agent_end(&self) -> io::Result<()> {
        self.set_terminal_progress(false)?;
        if self.working.loading_animation.borrow().is_some() {
            self.stop_working_loader();
        }
        if let Some((component, _)) = self.streaming.take() {
            let component: Rc<dyn Component> = component;
            self.chat_container.remove_child(&component);
        }
        drop(self.pending_tools.take());
        self.ui.request_render(false);
        Ok(())
    }

    /// Appends a pending row for the live `call` and registers it.
    ///
    /// # Errors
    /// Returns the failure of a renderer binding or of the row's rendering.
    fn add_pending_tool(
        &self,
        call: ToolCall,
    ) -> Result<Rc<ToolExecutionComponent>, Box<dyn Error>> {
        let row = self
            .render_tool_call(&call.id, &call.name, Some(call.arguments))
            .map_err(|error| -> Box<dyn Error> { error })?;
        self.pending_tools
            .borrow_mut()
            .insert(call.id, Rc::clone(&row));
        Ok(row)
    }

    /// The streaming assistant row, when a response streams.
    pub(super) fn streaming_component(&self) -> Option<Rc<AssistantMessageComponent>> {
        let streaming = self.streaming.borrow();
        streaming
            .as_ref()
            .map(|(component, _)| Rc::clone(component))
    }

    /// Shows or clears terminal progress when the settings show it.
    ///
    /// # Errors
    /// Returns the terminal's progress-writing failure.
    pub(super) fn set_terminal_progress(&self, active: bool) -> io::Result<()> {
        let session = self.runtime_host.session();
        if session
            .settings_manager()
            .borrow()
            .get_show_terminal_progress()
        {
            self.ui.terminal().borrow_mut().set_progress(active)?;
        }
        Ok(())
    }

    /// Sets the window title to the session's name, when it has one, and the lexical basename
    /// of the session's working directory.
    ///
    /// # Errors
    /// Returns the invalid stored name or the terminal's title-writing failure.
    pub(super) fn update_terminal_title(&self) -> Result<(), maestro_app::EventError> {
        let title = {
            let session = self.runtime_host.session();
            let manager = session.session_manager().borrow();
            let basename = maestro_path::basename(manager.get_cwd(), None);
            match manager.get_session_name()? {
                Some(name) => format!("{APP_TITLE} - {name} - {basename}"),
                None => format!("{APP_TITLE} - {basename}"),
            }
        };
        self.ui.terminal().borrow_mut().set_title(&title)?;
        Ok(())
    }
}
