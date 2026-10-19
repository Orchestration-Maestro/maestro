//! Coalesced next-frame assistant publication with independently retained children.
use crate::{
    browser::{attachment_content::AttachmentHost, timer::Timer},
    components::messages::{AssistantMessage, AssistantMessageProps},
    tools::types::{ToolContentOutput, ToolRenderView, ToolUiError},
};
use maestro_app::browser::components::messages::CustomAgentMessages as BrowserMessages;
use maestro_app::{AgentMessage, CustomAgentMessages, NoCustomMessages, ToolResultMessage};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::{Arc, RwLock},
};
mod cursor;

/// Borrowed policy for the currently published assistant row.
#[derive(Default)]
pub struct StreamingMessageProps<'a> {
    /// Available tool names.
    pub tools: &'a [String],
    /// Show the cursor and hide completed usage while streaming.
    pub is_streaming: bool,
    /// Calls awaiting completion.
    pub pending_tool_calls: Option<&'a HashSet<String>>,
    /// Shared completed results, with duplicate IDs already resolved.
    pub tool_results_by_id: Option<&'a HashMap<String, ToolResultMessage>>,
    /// Current usage activation.
    pub on_cost_click: Option<Rc<dyn Fn()>>,
}
/// Publication operands reachable by a weak frame callback.
struct StreamingState<C: CustomAgentMessages> {
    /// Most recent deferred input, read only at the frame boundary.
    pending: Option<AgentMessage<BrowserMessages<C>>>,
    /// Snapshot for deferred publication or original handle for immediate publication.
    published: Option<AgentMessage<BrowserMessages<C>>>,
    /// Whether this instance already owns an outstanding frame.
    scheduled: bool,
    /// Repaint owner learned from the first render.
    context: Option<egui::Context>,
}
/// One independently cancellable streaming assistant subtree.
pub struct StreamingMessageContainer<C: CustomAgentMessages = NoCustomMessages> {
    /// Existing native effect owner.
    host: Rc<dyn AttachmentHost>,
    /// Pending and published content, without a callback backedge.
    state: Rc<RefCell<StreamingState<C>>>,
    /// Cancel-on-drop registration for only this instance.
    frame: Option<Timer>,
    /// Retained assistant disclosure and text owners.
    assistant: Option<AssistantMessage>,
    /// Native animated cursor lifetime.
    cursor: Option<cursor::Cursor>,
}
impl<C: CustomAgentMessages + 'static> StreamingMessageContainer<C> {
    /// Bind an initially empty row to the existing platform host.
    #[must_use]
    pub fn new(host: Rc<dyn AttachmentHost>) -> Self {
        Self {
            host,
            state: Rc::new(RefCell::new(StreamingState {
                pending: None,
                published: None,
                scheduled: false,
                context: None,
            })),
            frame: None,
            assistant: None,
            cursor: None,
        }
    }
    /// Coalesce deferred updates or synchronously replace and revoke older work.
    /// Immediate publication retains the supplied shared message handle.
    /// # Errors
    /// Returns the original frame-registration failure.
    pub fn set_message(
        &mut self,
        message: Option<AgentMessage<BrowserMessages<C>>>,
        immediate: bool,
    ) -> Result<(), ToolUiError> {
        if immediate || message.is_none() {
            drop(self.frame.take());
            let mut state = self.state.borrow_mut();
            state.pending = None;
            state.scheduled = false;
            state.published = message;
            let clear = state.published.is_none();
            let context = state.context.clone();
            drop(state);
            if clear {
                self.assistant = None;
                self.cursor = None;
            }
            if let Some(context) = context {
                context.request_repaint();
            }
            return Ok(());
        }
        if !self.state.borrow().scheduled {
            let weak = Rc::downgrade(&self.state);
            self.frame = Some(self.host.animation_frame(Rc::new(move || {
                publish(&weak);
            }))?);
            self.state.borrow_mut().scheduled = true;
        }
        self.state.borrow_mut().pending = message;
        Ok(())
    }
    /// Draw only assistant content or the empty streaming placeholder.
    /// # Errors
    /// Returns reached child or native cursor failures, retaining opened overlays.
    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        props: StreamingMessageProps<'_>,
        view: ToolRenderView<'_>,
    ) -> ToolContentOutput {
        self.state.borrow_mut().context = Some(ui.ctx().clone());
        let mut overlays = Vec::new();
        let response = self.render_body(ui, props, (view, &mut overlays));
        ToolContentOutput { response, overlays }
    }
    /// Preserve the assistant child until its branch is removed or explicitly cleared.
    fn render_body(
        &mut self,
        ui: &mut egui::Ui,
        props: StreamingMessageProps<'_>,
        (view, overlays): (ToolRenderView<'_>, &mut Vec<crate::AttachmentOverlay>),
    ) -> Result<egui::Response, ToolUiError> {
        let published = self.state.borrow().published.clone();
        let show_cursor = props.is_streaming
            && (published.is_none() || matches!(published, Some(AgentMessage::Assistant(_))));
        if !show_cursor {
            self.cursor = None;
        }
        let row = ui.vertical(|ui| -> Result<(), ToolUiError> {
            ui.spacing_mut().item_spacing.y = 12.;
            if let Some(AgentMessage::Assistant(message)) = &published {
                let message = message
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone();
                let assistant = self
                    .assistant
                    .get_or_insert_with(|| AssistantMessage::new(Rc::clone(&self.host)));
                assistant
                    .render(
                        ui,
                        &message,
                        AssistantMessageProps {
                            tools: Some(props.tools),
                            pending_tool_calls: props.pending_tool_calls,
                            tool_results_by_id: props.tool_results_by_id,
                            is_streaming: props.is_streaming,
                            hide_tool_calls: false,
                            hide_pending_tool_calls: false,
                            on_cost_click: props.on_cost_click,
                        },
                        view,
                    )
                    .collect(overlays)?;
            } else {
                self.assistant = None;
            }
            if show_cursor {
                self.cursor
                    .get_or_insert_with(|| cursor::Cursor::new(Rc::clone(&self.host)))
                    .render(ui, view)?;
            }
            if published.is_none() && !props.is_streaming {
                return Ok(());
            }
            if published.is_none() || matches!(published, Some(AgentMessage::Assistant(_))) {
                ui.add_space(12.);
            }
            Ok(())
        });
        row.inner.map(|()| row.response)
    }
}
/// Snapshot nested assistant arguments at publication, without a JSON conversion.
fn publish<C: CustomAgentMessages>(weak: &std::rc::Weak<RefCell<StreamingState<C>>>) {
    let Some(state) = weak.upgrade() else { return };
    let mut state = state.borrow_mut();
    state.published = state.pending.take().map(|message| match message {
        AgentMessage::Assistant(value) => AgentMessage::Assistant(Arc::new(RwLock::new(
            value
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone(),
        ))),
        other => other,
    });
    state.scheduled = false;
    let context = state.context.clone();
    drop(state);
    if let Some(context) = context {
        context.request_repaint();
    }
}
