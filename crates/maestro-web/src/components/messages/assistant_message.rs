//! Ordered assistant content with emitted-position child ownership.
/// Native page assistant rows.
#[doc(hidden)]
pub mod dom;
use super::{AbortedMessage, ToolMessage, ToolMessageProps, color, reading_view};
use crate::{
    browser::attachment_content::AttachmentHost,
    components::{
        markdown_block::MarkdownBlock,
        native_text::{NativeText, TextKind},
        thinking_block::ThinkingBlock,
    },
    tools::types::{ToolContentOutput, ToolRenderView, ToolUiError},
};
use maestro_app::{AssistantContent, StopReason, ToolResultMessage};
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

/// Borrowed rendering policy, independent of the assistant record.
pub struct AssistantMessageProps<'a, D = serde_json::Value> {
    /// Available tool names in first-match order.
    pub tools: Option<&'a [String]>,
    /// Calls currently awaiting completion.
    pub pending_tool_calls: Option<&'a HashSet<String>>,
    /// Hide every tool call.
    pub hide_tool_calls: bool,
    /// Original result envelopes, including native details.
    pub tool_results_by_id: Option<&'a HashMap<String, ToolResultMessage<D>>>,
    /// Hide usage and shimmer thinking while streaming.
    pub is_streaming: bool,
    /// Hide pending calls only when no result exists.
    pub hide_pending_tool_calls: bool,
    /// Current usage action.
    pub on_cost_click: Option<Rc<dyn Fn()>>,
}
impl<D> Default for AssistantMessageProps<'_, D> {
    fn default() -> Self {
        Self {
            tools: None,
            pending_tool_calls: None,
            hide_tool_calls: false,
            tool_results_by_id: None,
            is_streaming: false,
            hide_pending_tool_calls: false,
            on_cost_click: None,
        }
    }
}
/// Typed widget occupying one emitted content position.
enum Child {
    /// Native rich text owner.
    Text(Box<MarkdownBlock>),
    /// Independent reasoning disclosure.
    Thinking(Box<ThinkingBlock>),
    /// Tool recipe state.
    Tool(Box<ToolMessage>),
}
impl Child {
    /// Match the emitted component kind, not a source index or call identity.
    fn matches(&self, chunk: &AssistantContent) -> bool {
        matches!(
            (self, chunk),
            (Self::Text(_), AssistantContent::Text(_))
                | (Self::Thinking(_), AssistantContent::Thinking(_))
                | (Self::Tool(_), AssistantContent::ToolCall(_))
        )
    }
    /// Construct the component selected by the current content.
    fn new(chunk: &AssistantContent, host: &Rc<dyn AttachmentHost>) -> Self {
        match chunk {
            AssistantContent::Text(_) => Self::Text(Box::new(MarkdownBlock::new(Rc::clone(host)))),
            AssistantContent::Thinking(_) => {
                Self::Thinking(Box::new(ThinkingBlock::new(Rc::clone(host))))
            }
            AssistantContent::ToolCall(_) => Self::Tool(Box::default()),
        }
    }
}
/// Retained assistant row with independently revocable child owners.
pub struct AssistantMessage {
    /// Platform effects for newly emitted children.
    host: Rc<dyn AttachmentHost>,
    /// Visible content positions only.
    children: Vec<Child>,
    /// Native usage, absent while streaming.
    usage: Option<NativeText>,
    /// Native literal error, absent outside nonempty errors.
    error: Option<NativeText>,
    /// Localized abort, absent outside aborted stops.
    abort: Option<AbortedMessage>,
}
impl AssistantMessage {
    /// Bind an empty assistant row to the supplied platform.
    #[must_use]
    pub fn new(host: Rc<dyn AttachmentHost>) -> Self {
        Self {
            host,
            children: Vec::new(),
            usage: None,
            error: None,
            abort: None,
        }
    }
    /// Render visible content in one pass, followed by usage and outcome text.
    /// # Errors
    /// Returns reached child, localization, dispatch or native publication failures.
    pub fn render<D: 'static>(
        &mut self,
        ui: &mut egui::Ui,
        message: &maestro_app::AssistantMessage,
        props: AssistantMessageProps<'_, D>,
        view: ToolRenderView<'_>,
    ) -> ToolContentOutput {
        let mut overlays = Vec::new();
        let response = self.render_body(ui, message, props, (view, &mut overlays));
        ToolContentOutput { response, overlays }
    }
    /// Draw the row while transferring opened owners before any later error.
    fn render_body<D: 'static>(
        &mut self,
        ui: &mut egui::Ui,
        message: &maestro_app::AssistantMessage,
        props: AssistantMessageProps<'_, D>,
        (view, overlays): (ToolRenderView<'_>, &mut Vec<crate::AttachmentOverlay>),
    ) -> Result<egui::Response, ToolUiError> {
        let row = ui.vertical(|ui| -> Result<(), ToolUiError> {
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(16, 0))
                .show(ui, |ui| self.content(ui, message, &props, (view, overlays)))
                .inner?;
            self.outcome(ui, message, props, view)
        });
        row.inner?;
        Ok(row.response)
    }
    /// Reconcile one ordered content pass, publishing replacements before retiring old owners.
    fn content<D: 'static>(
        &mut self,
        ui: &mut egui::Ui,
        message: &maestro_app::AssistantMessage,
        props: &AssistantMessageProps<'_, D>,
        (view, overlays): (ToolRenderView<'_>, &mut Vec<crate::AttachmentOverlay>),
    ) -> Result<(), ToolUiError> {
        let mut emitted = 0;
        ui.spacing_mut().item_spacing.y = 12.;
        for chunk in &message.content {
            if !visible(chunk, props) {
                continue;
            }
            let mut candidate = self
                .children
                .get(emitted)
                .is_none_or(|child| !child.matches(chunk))
                .then(|| Child::new(chunk, &self.host));
            let child = if let Some(child) = candidate.as_mut() {
                child
            } else {
                &mut self.children[emitted]
            };
            draw(
                child,
                chunk,
                (&message.stop_reason, props),
                ui,
                (view, overlays),
            )?;
            if let Some(child) = candidate {
                self.publish_child(emitted, child);
            }
            emitted += 1;
        }
        self.children.truncate(emitted);
        Ok(())
    }
    /// Replace or append one successfully published emitted component.
    fn publish_child(&mut self, position: usize, child: Child) {
        if let Some(slot) = self.children.get_mut(position) {
            *slot = child;
        } else {
            self.children.push(child);
        }
    }
    /// Reconcile usage and literal outcome owners independently of content positions.
    fn outcome<D>(
        &mut self,
        ui: &mut egui::Ui,
        message: &maestro_app::AssistantMessage,
        props: AssistantMessageProps<'_, D>,
        view: ToolRenderView<'_>,
    ) -> Result<(), ToolUiError> {
        if props.is_streaming {
            self.usage = None;
        } else {
            let text = crate::format_usage(Some(&message.usage));
            let usage = self
                .usage
                .get_or_insert_with(|| NativeText::new(Rc::clone(&self.host)));
            let clickable = props.on_cost_click.is_some();
            usage.set_action(props.on_cost_click);
            egui::Frame::new()
                .inner_margin(egui::Margin::symmetric(16, 0))
                .show(ui, |ui| {
                    ui.add_space(8.);
                    usage.render(ui, (&text, TextKind::Usage(clickable)), view)
                })
                .inner?;
        }
        if let Some(error) = message
            .error_message
            .as_deref()
            .filter(|text| message.stop_reason == StopReason::Error && !text.is_empty())
        {
            let prefix = view.i18n.text("Error:")?;
            let text = format!("{prefix} {error}");
            let native = self
                .error
                .get_or_insert_with(|| NativeText::new(Rc::clone(&self.host)));
            ui.add_space(12.);
            egui::Frame::new()
                .fill(color(view, "error")?.gamma_multiply(0.1))
                .corner_radius(8.)
                .inner_margin(12)
                .outer_margin(egui::Margin::symmetric(16, 0))
                .show(ui, |ui| {
                    native.render(ui, (&text, TextKind::Error(prefix.len())), view)
                })
                .inner?;
        } else {
            self.error = None;
        }
        if message.stop_reason == StopReason::Aborted {
            self.abort
                .get_or_insert_with(|| AbortedMessage::new(Rc::clone(&self.host)))
                .render(ui, view)?;
        } else {
            self.abort = None;
        }
        Ok(())
    }
}
/// Selected tool visibility, independent of the content representation.
struct Visibility {
    /// Hide every call.
    hide_all: bool,
    /// Hide pending calls without results.
    hide_pending: bool,
    /// Whether this exact call is pending without a paired result.
    pending_without_result: bool,
}
/// Apply shared admission without modifying the displayed source.
fn admitted(
    chunk: &AssistantContent,
    tool_state: impl FnOnce(&maestro_app::ToolCall) -> Visibility,
) -> bool {
    match chunk {
        AssistantContent::Text(text) => !text.text.chars().all(maestro_app::is_whitespace_scalar),
        AssistantContent::Thinking(text) => {
            !text.thinking.chars().all(maestro_app::is_whitespace_scalar)
        }
        AssistantContent::ToolCall(call) => {
            let state = tool_state(call);
            !(state.hide_all || state.hide_pending && state.pending_without_result)
        }
    }
}
/// Read only the selected call's visibility from the borrowed drawing policy.
fn visible<D>(chunk: &AssistantContent, props: &AssistantMessageProps<'_, D>) -> bool {
    admitted(chunk, |call| Visibility {
        hide_all: props.hide_tool_calls,
        hide_pending: props.hide_pending_tool_calls,
        pending_without_result: props
            .pending_tool_calls
            .is_some_and(|pending| pending.contains(&call.id))
            && props
                .tool_results_by_id
                .is_none_or(|results| !results.contains_key(&call.id)),
    })
}
/// Draw one admitted child, keeping native detail types unchanged.
fn draw<D: 'static>(
    child: &mut Child,
    chunk: &AssistantContent,
    policy: (&StopReason, &AssistantMessageProps<'_, D>),
    ui: &mut egui::Ui,
    (view, overlays): (ToolRenderView<'_>, &mut Vec<crate::AttachmentOverlay>),
) -> Result<(), ToolUiError> {
    let (stop, props) = policy;
    match (child, chunk) {
        (Child::Text(block), AssistantContent::Text(text)) => {
            block.content.clone_from(&text.text);
            block
                .render(ui, reading_view(view))
                .map_err(ToolUiError::reading)?;
        }
        (Child::Thinking(block), AssistantContent::Thinking(text)) => {
            block.content.clone_from(&text.thinking);
            block.is_streaming = props.is_streaming;
            block.render(ui, view)?;
        }
        (Child::Tool(block), AssistantContent::ToolCall(call)) => {
            let result = props
                .tool_results_by_id
                .and_then(|results| results.get(&call.id));
            block
                .render(
                    ui,
                    ToolMessageProps {
                        tool_call: call,
                        tool: props
                            .tools
                            .and_then(|tools| tools.iter().find(|tool| *tool == &call.name))
                            .map(String::as_str),
                        result,
                        pending: props
                            .pending_tool_calls
                            .is_some_and(|pending| pending.contains(&call.id)),
                        aborted: stop == &StopReason::Aborted && result.is_none(),
                        is_streaming: props.is_streaming,
                    },
                    view,
                )
                .collect(overlays)?;
        }
        _ => {
            return Err(ToolUiError::Presentation(
                "Message component kind does not match its content".into(),
            ));
        }
    }
    Ok(())
}
