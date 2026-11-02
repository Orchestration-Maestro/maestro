//! Native emitted-position placement with short application record borrows.
use super::{AssistantMessage, ToolResults};
use crate::{
    browser::attachment_content::browser::element,
    components::{
        markdown_block::MarkdownBlock, messages::ToolMessage, thinking_block::ThinkingBlock,
    },
    dom::{self, DomContent, DomRoot, DomView, MountedContent},
    tools::types::ToolUiError,
};
use maestro_app::{AssistantContent, StopReason, ToolCall, ToolResultMessage};
use std::{cell::RefCell, rc::Rc};
use web_sys::Element;

mod outcome;

/// Independently selected usage and stop outcome outside the model read guard.
struct Outcome {
    /// Original stop selection.
    stop: StopReason,
    /// Delegated usage, absent only while streaming.
    usage: Option<String>,
    /// Nonempty error source, present only for an error stop.
    error: Option<String>,
}

/// Mounted children indexed by emitted position rather than source position.
struct Placed {
    /// Ordered content container.
    body: Element,
    /// Independently retained content roots.
    children: Vec<MountedContent>,
    /// Independently retained usage and current action.
    usage: MountedContent,
    /// Literal error root, when selected.
    error: MountedContent,
    /// Localized abort root, when selected.
    abort: MountedContent,
}
impl<D: 'static> DomContent for AssistantMessage<D> {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        dom::install_styles(view).map_err(|error| ToolUiError::native(&error))?;
        let node = element(&view.document, "div", "maestro-assistant-row")?;
        let body = element(&view.document, "div", "maestro-assistant-parts")?;
        node.append_child(&body)
            .map_err(|error| ToolUiError::native(&error))?;
        let root = DomRoot {
            node: node.into(),
            owner: Some(Box::new(RefCell::new(Placed {
                body,
                children: Vec::new(),
                usage: MountedContent::default(),
                error: MountedContent::default(),
                abort: MountedContent::default(),
            }))),
        };
        self.update(&root, view)?;
        Ok(root)
    }
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let (chunks, outcome) = self.operand()?;
        let placed = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<RefCell<Placed>>())
            .ok_or_else(|| ToolUiError::Presentation("Missing assistant row state".into()))?;
        let mut placed = placed.borrow_mut();
        let Placed { body, children, .. } = &mut *placed;
        let count = chunks.len();
        for (index, chunk) in chunks.into_iter().enumerate() {
            let child = self.child(chunk, &outcome.stop)?;
            if index == children.len() {
                children.push(MountedContent::default());
            }
            children[index].show(child.as_ref(), body, view)?;
        }
        for child in &mut children[count..] {
            child.clear();
        }
        children.truncate(count);
        self.outcome(root, &mut placed, outcome, view)
    }
}
impl<D: 'static> AssistantMessage<D> {
    /// Leave the model guard with only operation-local selected operands, never a retained copy.
    fn operand(&self) -> Result<(Vec<AssistantContent>, Outcome), ToolUiError> {
        let message = self
            .message
            .as_ref()
            .ok_or_else(|| ToolUiError::Presentation("Missing assistant message".into()))?
            .try_read()
            .map_err(|_| ToolUiError::Presentation("Message borrow conflict".into()))?;
        let outcome = Outcome {
            stop: message.stop_reason.clone(),
            usage: (!self.is_streaming).then(|| crate::format_usage(Some(&message.usage))),
            error: message
                .error_message
                .as_ref()
                .filter(|text| message.stop_reason == StopReason::Error && !text.is_empty())
                .cloned(),
        };
        Ok((
            self.parts(message.content.iter()).cloned().collect(),
            outcome,
        ))
    }
    /// Select native child content without retaining any application read guard.
    fn child(
        &self,
        chunk: AssistantContent,
        stop: &StopReason,
    ) -> Result<Box<dyn DomContent>, ToolUiError> {
        Ok(match chunk {
            AssistantContent::Text(text) => {
                let mut block = MarkdownBlock::default();
                block.content = text.text;
                Box::new(block)
            }
            AssistantContent::Thinking(text) => Box::new(ThinkingBlock {
                content: text.thinking,
                is_streaming: self.is_streaming,
            }),
            AssistantContent::ToolCall(call) => self.tool(Rc::new(call), stop)?,
        })
    }
    /// Resolve only this reached envelope; live standard records can change between siblings.
    fn tool(
        &self,
        call: Rc<ToolCall>,
        stop: &StopReason,
    ) -> Result<Box<dyn DomContent>, ToolUiError> {
        match self.tool_results_by_id.as_ref() {
            Some(ToolResults::Live(results)) => {
                let result = results
                    .get(&call.id)
                    .map(|result| {
                        result
                            .try_read()
                            .map(|result| Rc::new(result.clone()))
                            .map_err(|_| ToolUiError::Presentation("Result borrow conflict".into()))
                    })
                    .transpose()?;
                Ok(self.tool_row(call, result, stop))
            }
            results => {
                let result = results.and_then(|results| match results {
                    ToolResults::Native(results) => results.get(&call.id).cloned(),
                    ToolResults::Live(_) => None,
                });
                Ok(self.tool_row(call, result, stop))
            }
        }
    }
    /// Keep native details unchanged while forwarding the independently selected flags.
    fn tool_row<T: 'static>(
        &self,
        call: Rc<ToolCall>,
        result: Option<Rc<ToolResultMessage<T>>>,
        stop: &StopReason,
    ) -> Box<dyn DomContent> {
        let pending = self
            .pending_tool_calls
            .as_ref()
            .is_some_and(|ids| ids.contains(&call.id));
        let aborted = stop == &StopReason::Aborted && result.is_none();
        Box::new(ToolMessage {
            tool_call: Some(call),
            tool: None,
            result,
            pending,
            aborted,
            is_streaming: self.is_streaming,
        })
    }
}
