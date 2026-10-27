//! Frame-deferred publication of the streamed message into its mounted branch.
use super::{LiveResults, Message, StreamingMessageContainer, Tools};
use crate::{
    browser::{attachment_content::browser::element, timer::Timer},
    components::messages::{AssistantMessage, ToolResults},
    dom::{self, DomContent, DomRoot, DomView, MountedContent},
    tools::types::ToolUiError,
};
use maestro_app::{AgentMessage, CustomAgentMessages};
use std::{
    cell::RefCell,
    collections::HashSet,
    rc::{Rc, Weak},
    sync::{Arc, PoisonError, RwLock},
};
use web_sys::{Element, Node};

/// The published and pending messages, the queued frame and the mounted root.
pub(super) struct Stream<C: CustomAgentMessages> {
    /// The message the mounted root shows.
    message: Option<Message<C>>,
    /// The latest deferred message, published on the queued frame.
    pending: Option<Message<C>>,
    /// The queued frame; dropping it retires the work.
    frame: Option<Timer>,
    /// The root showing `message`, absent while unmounted or released.
    mounted: Option<Mounted>,
}
impl<C: CustomAgentMessages> Default for Stream<C> {
    fn default() -> Self {
        Self {
            message: None,
            pending: None,
            frame: None,
            mounted: None,
        }
    }
}
/// One mounted root with the view and settings its branch uses.
struct Mounted {
    /// The container root.
    node: Node,
    /// Page operands for mounting the branch.
    view: DomView,
    /// Container settings for the next branch.
    operands: Operands,
    /// The branch placed under `node`.
    branch: MountedContent,
    /// Identity shared with the root's release guard.
    token: Rc<()>,
}
/// The container settings forwarded to the branch.
#[derive(Clone)]
struct Operands {
    /// Local tools.
    tools: Tools,
    /// Whether the response is streaming.
    is_streaming: bool,
    /// Calls waiting for results.
    pending_tool_calls: Option<Rc<HashSet<String>>>,
    /// Live result envelopes.
    tool_results_by_id: Option<LiveResults>,
    /// Usage action.
    on_cost_click: Option<Rc<dyn Fn()>>,
}
/// Releases a mounted root's claim on the stream when its root is dropped.
struct Guard<C: CustomAgentMessages> {
    /// The stream the root was mounted from.
    stream: Weak<RefCell<Stream<C>>>,
    /// The mount this root belongs to.
    token: Rc<()>,
}
impl<C: CustomAgentMessages> Drop for Guard<C> {
    fn drop(&mut self) {
        let Some(stream) = self.stream.upgrade() else {
            return;
        };
        let Ok(mut stream) = stream.try_borrow_mut() else {
            return;
        };
        if (stream.mounted.as_ref()).is_some_and(|mounted| Rc::ptr_eq(&mounted.token, &self.token))
        {
            let released = stream.mounted.take();
            drop(stream);
            drop(released);
        }
    }
}

impl<C: CustomAgentMessages + 'static> StreamingMessageContainer<C> {
    /// Show `message` on the next animation frame, or at once when `immediate` is set or the
    /// message is `None`; publishing at once retires any queued frame.
    /// # Errors
    /// Returns a failure to queue the frame or to show an immediate message.
    pub fn set_message(
        &self,
        message: Option<Message<C>>,
        immediate: bool,
    ) -> Result<(), ToolUiError> {
        let mut stream = self.state.borrow_mut();
        if immediate || message.is_none() {
            let retired = stream.frame.take();
            stream.pending = None;
            stream.message = message;
            if let Some(mounted) = &mut stream.mounted {
                mounted.operands = self.operands();
            }
            drop(stream);
            drop(retired);
            return publish(&self.state);
        }
        stream.pending = message;
        if stream.frame.is_none() {
            stream.frame = Some(queue(Rc::downgrade(&self.state))?);
        }
        Ok(())
    }
    /// The current settings, shared with the branch.
    fn operands(&self) -> Operands {
        Operands {
            tools: Rc::clone(&self.tools),
            is_streaming: self.is_streaming,
            pending_tool_calls: self.pending_tool_calls.clone(),
            tool_results_by_id: self.tool_results_by_id.clone(),
            on_cost_click: self.on_cost_click.clone(),
        }
    }
}

/// Register the next animation frame for the deferred publication.
fn queue<C: CustomAgentMessages + 'static>(
    stream: Weak<RefCell<Stream<C>>>,
) -> Result<Timer, ToolUiError> {
    let window =
        web_sys::window().ok_or_else(|| ToolUiError::Presentation("Missing window".into()))?;
    let publish = move || stream.upgrade().iter().for_each(deferred);
    crate::browser::timer::frame(window, Rc::new(publish))
        .map_err(|error| ToolUiError::native(&error))
}

/// Publish the queued message as a copy of its content taken now.
fn deferred<C: CustomAgentMessages + 'static>(stream: &Rc<RefCell<Stream<C>>>) {
    let fired = {
        let mut state = stream.borrow_mut();
        let Some(pending) = state.pending.take() else {
            return;
        };
        state.message = Some(snapshot(&pending));
        state.frame.take()
    };
    // A frame has no return channel; a failed branch keeps the last published content.
    let _ = publish(stream);
    drop(fired);
}

/// A copy of an assistant message's content; other messages keep their handle, as the stream
/// shows nothing for them.
fn snapshot<C: CustomAgentMessages>(message: &Message<C>) -> Message<C> {
    match message {
        AgentMessage::Assistant(assistant) => {
            let content = assistant
                .read()
                .unwrap_or_else(PoisonError::into_inner)
                .clone();
            AgentMessage::Assistant(Arc::new(RwLock::new(content)))
        }
        other => other.clone(),
    }
}

/// Show the published message in the mounted root, without holding the stream while rows
/// prepare their content.
fn publish<C: CustomAgentMessages + 'static>(
    stream: &Rc<RefCell<Stream<C>>>,
) -> Result<(), ToolUiError> {
    let (message, mut mounted) = {
        let mut state = stream.borrow_mut();
        let Some(mounted) = state.mounted.take() else {
            return Ok(());
        };
        (state.message.clone(), mounted)
    };
    let result = mounted.show(message.as_ref());
    let mut state = stream.borrow_mut();
    if state.mounted.is_none() {
        state.mounted = Some(mounted);
    }
    result
}

impl Mounted {
    /// Place the branch for `message`: the assistant row, the pulse alone, or nothing.
    fn show<C: CustomAgentMessages>(
        &mut self,
        message: Option<&Message<C>>,
    ) -> Result<(), ToolUiError> {
        let streaming = self.operands.is_streaming;
        let assistant = match message {
            Some(AgentMessage::Assistant(assistant)) => {
                Some(AssistantMessage::<serde_json::Value> {
                    message: Some(Arc::clone(assistant)),
                    tools: Some(Rc::clone(&self.operands.tools)),
                    pending_tool_calls: self.operands.pending_tool_calls.clone(),
                    hide_tool_calls: false,
                    tool_results_by_id: (self.operands.tool_results_by_id.clone())
                        .map(ToolResults::Live),
                    is_streaming: streaming,
                    hide_pending_tool_calls: false,
                    on_cost_click: self.operands.on_cost_click.clone(),
                })
            }
            None if streaming => None,
            Some(_) | None => {
                self.branch.clear();
                return Ok(());
            }
        };
        let branch = Branch {
            assistant,
            pulse: streaming,
        };
        self.branch.show(&branch, &self.node, &self.view)
    }
}

/// The shown branch: an optional assistant row followed by the pulse while streaming.
struct Branch {
    /// The streamed assistant row.
    assistant: Option<AssistantMessage>,
    /// Whether the response is still streaming.
    pulse: bool,
}
/// A mounted branch's assistant placement and pulse.
struct Placed {
    /// The assistant row's placement, kept while the branch stays mounted.
    assistant: RefCell<MountedContent>,
    /// The pulse element.
    pulse: Element,
}
impl DomContent for Branch {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let root = DomRoot {
            node: element(&view.document, "div", "maestro-stream-content")?.into(),
            owner: Some(Box::new(Placed {
                assistant: RefCell::default(),
                pulse: element(&view.document, "span", "maestro-stream-pulse")?,
            })),
        };
        self.update(&root, view)?;
        Ok(root)
    }
    /// Show or remove the pulse, then update the assistant row in place before it.
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let placed = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<Placed>())
            .ok_or_else(|| ToolUiError::Presentation("Missing stream state".into()))?;
        if self.pulse {
            root.node
                .append_child(&placed.pulse)
                .map_err(|error| ToolUiError::native(&error))?;
        } else {
            placed.pulse.remove();
        }
        let before = self.pulse.then_some(&*placed.pulse);
        let mut assistant = placed.assistant.borrow_mut();
        match &self.assistant {
            Some(row) => assistant.show_before(row, &root.node, before, view)?,
            None => assistant.clear(),
        }
        Ok(())
    }
}

impl<C: CustomAgentMessages + 'static> DomContent for StreamingMessageContainer<C> {
    /// Mount the container root and show the published message; the root becomes the one
    /// later publications update.
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        dom::install_styles(view).map_err(|error| ToolUiError::native(&error))?;
        let node: Node = element(&view.document, "div", "maestro-stream")?.into();
        let token = Rc::new(());
        let previous = self.state.borrow_mut().mounted.replace(Mounted {
            node: node.clone(),
            view: view.clone(),
            operands: self.operands(),
            branch: MountedContent::default(),
            token: Rc::clone(&token),
        });
        drop(previous);
        publish(&self.state)?;
        Ok(DomRoot {
            node,
            owner: Some(Box::new(Guard {
                stream: Rc::downgrade(&self.state),
                token,
            })),
        })
    }
    /// Apply the current settings to the mounted root and show the published message again.
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let token = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<Guard<C>>())
            .map(|guard| Rc::clone(&guard.token))
            .ok_or_else(|| ToolUiError::Presentation("Missing stream state".into()))?;
        {
            let mut stream = self.state.borrow_mut();
            let mounted = (stream.mounted.as_mut())
                .filter(|mounted| Rc::ptr_eq(&mounted.token, &token))
                .ok_or_else(|| ToolUiError::Presentation("Released stream root".into()))?;
            mounted.operands = self.operands();
            mounted.view = view.clone();
        }
        publish(&self.state)
    }
}
