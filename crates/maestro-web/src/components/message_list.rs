//! Stable emitted-position transcript rows with one result index per refresh.
use crate::{
    Attachment,
    browser::attachment_content::AttachmentHost,
    components::{
        message_renderer_registry::render_message,
        messages::{AssistantMessage, AssistantMessageProps, UserMessage, UserMessageInput},
    },
    tools::types::{ToolContent, ToolContentOutput, ToolRenderState, ToolRenderView, ToolUiError},
};
use maestro_app::browser::components::messages::{
    CustomAgentMessages as BrowserMessages, is_artifact_message,
};
use maestro_app::{AgentMessage, CustomAgentMessages, NoCustomMessages, ToolResultMessage};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

/// Borrowed stable transcript inputs for one refresh.
pub struct MessageListProps<'a, C: CustomAgentMessages = NoCustomMessages> {
    /// Ordered conversation records, including results and persistence entries.
    pub messages: &'a [AgentMessage<BrowserMessages<C>>],
    /// Available tool names.
    pub tools: &'a [String],
    /// Tool calls awaiting completion.
    pub pending_tool_calls: Option<&'a HashSet<String>>,
    /// Hide pending calls while the conversation streams.
    pub is_streaming: bool,
    /// Current usage action.
    pub on_cost_click: Option<Rc<dyn Fn()>>,
}
impl<C: CustomAgentMessages> Default for MessageListProps<'_, C> {
    fn default() -> Self {
        Self {
            messages: &[],
            tools: &[],
            pending_tool_calls: None,
            is_streaming: false,
            on_cost_click: None,
        }
    }
}
/// Prepared operands independent of any message guard.
enum Item {
    /// Standard user content or browser attachment content.
    User(maestro_app::UserContent, Option<Vec<Attachment>>),
    /// Completed assistant content.
    Assistant(Box<maestro_app::AssistantMessage>),
    /// Original callback's owned recipe.
    Custom(Box<dyn ToolContent>),
}
/// One emitted component's retained state.
enum Row {
    /// User widget and stable attachment handles.
    User(Box<UserMessage>, Vec<Rc<RefCell<Attachment>>>),
    /// Assistant widgets retained until their kind changes.
    Assistant(Box<AssistantMessage>),
    /// Supplied recipe's typed widget state.
    Custom(ToolRenderState),
}
impl Row {
    /// Retain only a component whose kind matches the emitted item.
    fn matches(&self, item: &Item) -> bool {
        matches!(
            (self, item),
            (Self::User(..), Item::User(..))
                | (Self::Assistant(_), Item::Assistant(_))
                | (Self::Custom(_), Item::Custom(_))
        )
    }
    /// Create the component selected at this position.
    fn new(item: &Item, host: &Rc<dyn AttachmentHost>) -> Self {
        match item {
            Item::User(..) => Self::User(Box::new(UserMessage::new(Rc::clone(host))), Vec::new()),
            Item::Assistant(_) => Self::Assistant(Box::new(AssistantMessage::new(Rc::clone(host)))),
            Item::Custom(_) => Self::Custom(ToolRenderState::default()),
        }
    }
}
/// Stable transcript rows retained by emitted position.
pub struct MessageList<C: CustomAgentMessages = NoCustomMessages> {
    /// Platform owner used for retained children.
    host: Rc<dyn AttachmentHost>,
    /// Component state for each emitted position.
    rows: Vec<Row>,
    /// Prepared recipes retained between transcript refreshes.
    items: Vec<Item>,
    /// Shared last-result-wins operands for stable and streaming consumers.
    results: HashMap<String, ToolResultMessage>,
    /// Custom message vocabulary without ownership or serialization bounds.
    custom: std::marker::PhantomData<fn() -> C>,
}
impl<C: CustomAgentMessages + 'static> MessageList<C> {
    /// Bind an empty transcript to its existing platform host.
    #[must_use]
    pub fn new(host: Rc<dyn AttachmentHost>) -> Self {
        Self {
            host,
            rows: Vec::new(),
            items: Vec::new(),
            results: HashMap::new(),
            custom: std::marker::PhantomData,
        }
    }
    /// Render completed rows, excluding artifacts before custom dispatch.
    /// # Errors
    /// Returns original recipe preparation, child drawing or platform failures.
    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        props: MessageListProps<'_, C>,
        view: ToolRenderView<'_>,
    ) -> ToolContentOutput {
        let mut overlays = Vec::new();
        let response = self.render_body(ui, props, (view, &mut overlays));
        ToolContentOutput { response, overlays }
    }
    /// Finish the index pass before preparing callbacks in message order.
    fn render_body(
        &mut self,
        ui: &mut egui::Ui,
        props: MessageListProps<'_, C>,
        (view, overlays): (ToolRenderView<'_>, &mut Vec<crate::AttachmentOverlay>),
    ) -> Result<egui::Response, ToolUiError> {
        self.refresh(props.messages)?;
        let row = ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 12.;
            let mut emitted = 0;
            for position in 0..self.items.len() {
                self.draw_row(ui, position, &props, (view, overlays))?;
                emitted += 1;
            }
            self.rows.truncate(emitted);
            Ok(())
        });
        drop(props);
        row.inner.map(|()| row.response)
    }
    /// Prepare native/custom content once for both transcript consumers.
    pub(crate) fn refresh(
        &mut self,
        messages: &[AgentMessage<BrowserMessages<C>>],
    ) -> Result<(), ToolUiError> {
        let results: HashMap<String, ToolResultMessage> = messages
            .iter()
            .filter_map(|message| {
                let AgentMessage::ToolResult(value) = message else {
                    return None;
                };
                let result = value
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                Some((result.tool_call_id.clone(), result.clone()))
            })
            .collect();
        let items = messages
            .iter()
            .filter_map(|message| prepare(message).transpose())
            .collect::<Result<Vec<_>, _>>()?;
        self.items = items;
        self.results = results;
        self.rows.truncate(self.items.len());
        Ok(())
    }
    /// Number of emitted positions, including empty custom recipes.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn len(&self) -> usize {
        self.items.len()
    }
    /// One shared result map for the stable list and the assistant in progress.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn results(&self) -> &HashMap<String, ToolResultMessage> {
        &self.results
    }
    /// Publish a changed component only after its first successful drawing.
    pub(crate) fn draw_row(
        &mut self,
        ui: &mut egui::Ui,
        position: usize,
        props: &MessageListProps<'_, C>,
        output: (ToolRenderView<'_>, &mut Vec<crate::AttachmentOverlay>),
    ) -> Result<(), ToolUiError> {
        let item = &self.items[position];
        let mut candidate = self
            .rows
            .get(position)
            .is_none_or(|row| !row.matches(item))
            .then(|| Row::new(item, &self.host));
        let row = if let Some(row) = candidate.as_mut() {
            row
        } else {
            &mut self.rows[position]
        };
        draw(row, item, ui, (props, &self.results), output)?;
        if let Some(row) = candidate {
            match self.rows.get_mut(position) {
                Some(slot) => *slot = row,
                None => self.rows.push(row),
            }
        }
        Ok(())
    }
}
/// Filter persistence records before running any registered callback.
fn prepare<C: CustomAgentMessages + 'static>(
    message: &AgentMessage<BrowserMessages<C>>,
) -> Result<Option<Item>, ToolUiError> {
    if is_artifact_message(message) {
        return Ok(None);
    }
    if let Some(content) = render_message(message)? {
        return Ok(Some(Item::Custom(content)));
    }
    Ok(match message {
        AgentMessage::User(value) => Some(Item::User(
            value
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .content
                .clone(),
            None,
        )),
        AgentMessage::Assistant(value) => Some(Item::Assistant(Box::new(
            value
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone(),
        ))),
        AgentMessage::Custom(value) => match &*value
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            BrowserMessages::UserWithAttachments(user) => {
                Some(Item::User(user.content.clone(), user.attachments.clone()))
            }
            BrowserMessages::Artifact(_) | BrowserMessages::Other(_) => None,
        },
        AgentMessage::ToolResult(_) => None,
    })
}
/// Draw one prepared row, transferring overlays before observing a later error.
fn draw<C: CustomAgentMessages>(
    row: &mut Row,
    item: &Item,
    ui: &mut egui::Ui,
    (props, results): (
        &MessageListProps<'_, C>,
        &HashMap<String, ToolResultMessage>,
    ),
    (view, overlays): (ToolRenderView<'_>, &mut Vec<crate::AttachmentOverlay>),
) -> Result<(), ToolUiError> {
    let output = match (row, item) {
        (Row::Custom(state), Item::Custom(content)) => state.draw(content.as_ref(), ui, view),
        (Row::Assistant(widget), Item::Assistant(message)) => widget.render(
            ui,
            message,
            AssistantMessageProps {
                tools: Some(props.tools),
                pending_tool_calls: props.pending_tool_calls,
                hide_tool_calls: false,
                tool_results_by_id: Some(results),
                is_streaming: false,
                hide_pending_tool_calls: props.is_streaming,
                on_cost_click: props.on_cost_click.clone(),
            },
            view,
        ),
        (Row::User(widget, retained), Item::User(content, attachments)) => {
            let attachments = attachments.as_deref().unwrap_or_default();
            retained.truncate(attachments.len());
            for (index, attachment) in attachments.iter().enumerate() {
                if let Some(slot) = retained.get(index) {
                    *slot.borrow_mut() = attachment.clone();
                } else {
                    retained.push(Rc::new(RefCell::new(attachment.clone())));
                }
            }
            widget.render(
                ui,
                UserMessageInput::WithAttachments {
                    content,
                    attachments: Some(retained),
                },
                view,
            )
        }
        _ => {
            return Err(ToolUiError::Presentation(
                "Transcript component kind does not match its content".into(),
            ));
        }
    };
    output.collect(overlays)?;
    Ok(())
}
