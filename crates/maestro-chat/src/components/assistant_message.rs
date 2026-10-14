//! Ordered assistant content with retained-message thinking controls.
use maestro_app::{
    AssistantContent, AssistantMessage, SharedAssistantMessage, StopReason, is_whitespace_scalar,
};
use maestro_theme::{LiveTheme, Theme, ThemeColor};
use maestro_tui::{Component, Container, MarkdownTheme, Spacer, TerminalImage, Text};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// Initial thinking visibility and label decoration.
pub struct AssistantMessageOptions {
    /// Replaces visible thinking with the configured label.
    pub hide_thinking_block: bool,
    /// Literal label shown for each hidden thinking block.
    pub hidden_thinking_label: String,
    /// Enables italic decoration on the hidden label.
    pub styles_enabled: bool,
}
impl Default for AssistantMessageOptions {
    fn default() -> Self {
        Self {
            hide_thinking_block: false,
            hidden_thinking_label: "Thinking...".into(),
            styles_enabled: true,
        }
    }
}
/// Assistant content inside an editable outer container.
pub struct AssistantMessageComponent {
    /// Inherited editable children.
    container: Container,
    /// Retained inner content, even if removed from the outer array.
    content: Rc<Container>,
    /// Aliases the caller's live message.
    message: RefCell<Option<SharedAssistantMessage>>,
    /// Controls read afresh during rebuilding.
    options: RefCell<AssistantMessageOptions>,
    /// Shared rendering context.
    rendering: super::MessageRendering,
    /// Whether the last content update contained a tool call.
    has_tools: Cell<bool>,
}
impl AssistantMessageComponent {
    /// Builds initial children, permitting no initial message.
    #[must_use]
    pub fn new(
        message: Option<SharedAssistantMessage>,
        options: AssistantMessageOptions,
        theme: LiveTheme,
        markdown_theme: Rc<MarkdownTheme>,
        terminal_image: TerminalImage,
    ) -> Self {
        let container = Container::new();
        let content = Rc::new(Container::new());
        container.add_child(content.clone());
        let result = Self {
            container,
            content,
            message: RefCell::new(message),
            options: RefCell::new(options),
            rendering: super::MessageRendering::new(theme, markdown_theme, terminal_image),
            has_tools: Cell::new(false),
        };
        result.rebuild();
        result
    }
    /// Replaces the retained handle and inner content, keeping outer edits.
    pub fn update_content(&self, message: SharedAssistantMessage) {
        let previous = self.message.replace(Some(message));
        drop(previous);
        self.rebuild();
    }
    /// Changes thinking visibility and rebuilds from the retained message.
    pub fn set_hide_thinking_block(&self, hide: bool) {
        self.options.borrow_mut().hide_thinking_block = hide;
        self.rebuild();
    }
    /// Changes the hidden label and rebuilds even when thinking is visible.
    pub fn set_hidden_thinking_label(&self, label: String) {
        self.options.borrow_mut().hidden_thinking_label = label;
        self.rebuild();
    }
    /// Copies only child-owned text while holding the message read guard.
    fn rebuild(&self) {
        let Some(message) = self.message.borrow().clone() else {
            return;
        };
        let (blocks, tools, notice) = {
            let message = message
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let blocks: Vec<_> = message
                .content
                .iter()
                .filter_map(Visible::from_content)
                .collect();
            let tools = message
                .content
                .iter()
                .any(|block| matches!(block, AssistantContent::ToolCall(_)));
            let notice = (!tools).then(|| notice(&message)).flatten();
            (blocks, tools, notice)
        };
        self.content.clear();
        self.has_tools.set(tools);
        if !blocks.is_empty() {
            self.content.add_child(Rc::new(Spacer::new(1)));
        }
        let count = blocks.len();
        for (index, block) in blocks.into_iter().enumerate() {
            let thinking = matches!(block, Visible::Thinking(_));
            self.content.add_child(self.block(block));
            if thinking && index + 1 < count {
                self.content.add_child(Rc::new(Spacer::new(1)));
            }
        }
        if let Some(notice) = notice {
            self.content.add_child(Rc::new(Spacer::new(1)));
            let text = self.rendering.paint(&ThemeColor::Error, &notice);
            self.content.add_child(Rc::new(Text::new(text, 1, 0, None)));
        }
    }
    /// Builds one visible child without holding a message guard or option borrow.
    fn block(&self, block: Visible) -> Rc<dyn Component> {
        match block {
            Visible::Text(text) => self.rendering.markdown(text, 1, None, false),
            Visible::Thinking(text) => {
                let options = self.options.borrow();
                if options.hide_thinking_block {
                    let label = options.hidden_thinking_label.clone();
                    let enabled = options.styles_enabled;
                    drop(options);
                    let label = self.rendering.paint(&ThemeColor::ThinkingText, &label);
                    Rc::new(Text::new(Theme::italic(&label, enabled), 1, 0, None))
                } else {
                    drop(options);
                    self.rendering
                        .markdown(text, 1, Some(ThemeColor::ThinkingText), true)
                }
            }
        }
    }
}
/// Child-owned visible text; tool calls do not produce a child here.
enum Visible {
    /// Ordinary assistant Markdown.
    Text(String),
    /// Thinking Markdown or its hidden label.
    Thinking(String),
}
impl Visible {
    /// Trims with the shared scalar predicate before testing visibility.
    fn from_content(content: &AssistantContent) -> Option<Self> {
        let (text, constructor): (&str, fn(String) -> Self) = match content {
            AssistantContent::Text(value) => (&value.text, Self::Text),
            AssistantContent::Thinking(value) => (&value.thinking, Self::Thinking),
            AssistantContent::ToolCall(_) => return None,
        };
        let text = text.trim_matches(is_whitespace_scalar);
        (!text.is_empty()).then(|| constructor(text.to_owned()))
    }
}
/// Selects a failure notice without changing whitespace-only custom errors.
fn notice(message: &AssistantMessage) -> Option<String> {
    let error = message
        .error_message
        .as_deref()
        .filter(|text| !text.is_empty());
    match message.stop_reason {
        StopReason::Aborted => Some(
            error
                .filter(|text| *text != "Request was aborted")
                .unwrap_or("Operation aborted")
                .into(),
        ),
        StopReason::Error => Some(format!("Error: {}", error.unwrap_or("Unknown error"))),
        _ => None,
    }
}
message_children!(AssistantMessageComponent, Container, container);
impl Component for AssistantMessageComponent {
    fn render(&self, width: usize) -> Vec<String> {
        let lines = self.container.render(width);
        if self.has_tools.get() {
            lines
        } else {
            super::mark_zone(lines)
        }
    }
    fn invalidate(&self) {
        self.container.invalidate();
        self.rebuild();
    }
}
