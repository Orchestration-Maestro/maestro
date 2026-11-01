//! Ordered assistant content with retained-message thinking controls.
use maestro_app::{AssistantContent, SharedAssistantMessage, StopReason, is_whitespace_scalar};
use maestro_theme::{LiveTheme, Theme, ThemeColor};
use maestro_tui::{Component, Container, MarkdownTheme, Spacer, TerminalImage, Text};
use serde_json::Value;
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
    /// The live or recorded message the content is read from.
    source: RefCell<Option<Source>>,
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
            source: RefCell::new(message.map(Source::Live)),
            options: RefCell::new(options),
            rendering: super::MessageRendering {
                clean_content: true,
                ..super::MessageRendering::new(theme, markdown_theme, terminal_image)
            },
            has_tools: Cell::new(false),
        };
        result.rebuild();
        result
    }
    /// Builds initial children from a stored assistant record, reading only the content and
    /// failure fields it displays, so tool arguments keep every JSON value they were stored
    /// with.
    #[must_use]
    pub fn from_record(
        record: Rc<Value>,
        options: AssistantMessageOptions,
        theme: LiveTheme,
        markdown_theme: Rc<MarkdownTheme>,
        terminal_image: TerminalImage,
    ) -> Self {
        let component = Self::new(None, options, theme, markdown_theme, terminal_image);
        component.source.replace(Some(Source::Recorded(record)));
        component.rebuild();
        component
    }
    /// Replaces the retained message, releasing a recorded one, and the inner content,
    /// keeping outer edits.
    pub fn update_content(&self, message: SharedAssistantMessage) {
        let previous = self.source.replace(Some(Source::Live(message)));
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
    /// Copies Markdown text under the message guard, omitting hidden thinking bodies.
    fn rebuild(&self) {
        let Some(source) = self.source.borrow().clone() else {
            return;
        };
        let hide = self.options.borrow().hide_thinking_block;
        let (blocks, tools, notice) = match source {
            Source::Live(message) => {
                let message = message
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let failure = match message.stop_reason {
                    StopReason::Aborted => Some(Failure::Aborted),
                    StopReason::Error => Some(Failure::Error),
                    StopReason::Length => Some(Failure::Length),
                    _ => None,
                };
                let content = message.content.iter().map(|content| match content {
                    AssistantContent::Text(value) => Shown::Text(&value.text),
                    AssistantContent::Thinking(value) => Shown::Thinking(&value.thinking),
                    AssistantContent::ToolCall(_) => Shown::ToolCall,
                });
                select(content, hide, failure, message.error_message.as_deref())
            }
            Source::Recorded(record) => {
                let failure = match record.get("stopReason").and_then(Value::as_str) {
                    Some("aborted") => Some(Failure::Aborted),
                    Some("error") => Some(Failure::Error),
                    Some("length") => Some(Failure::Length),
                    _ => None,
                };
                let content = record.get("content").and_then(Value::as_array);
                let content = content.into_iter().flatten().filter_map(Shown::from_record);
                let error = record.get("errorMessage").and_then(Value::as_str);
                select(content, hide, failure, error)
            }
        };
        self.content.clear();
        self.has_tools.set(tools);
        if !blocks.is_empty() {
            self.content.add_child(Rc::new(Spacer::new(1)));
        }
        let count = blocks.len();
        for (index, block) in blocks.into_iter().enumerate() {
            let thinking = matches!(block, Visible::Thinking(_) | Visible::HiddenThinking);
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
    /// Builds one visible child after releasing the message guard.
    fn block(&self, block: Visible) -> Rc<dyn Component> {
        match block {
            Visible::Text(text) => self.rendering.markdown(text, 1, None, false),
            Visible::Thinking(text) => {
                self.rendering
                    .markdown(text, 1, Some(ThemeColor::ThinkingText), true)
            }
            Visible::HiddenThinking => {
                let options = self.options.borrow();
                let label = self
                    .rendering
                    .paint(&ThemeColor::ThinkingText, &options.hidden_thinking_label);
                let enabled = options.styles_enabled;
                drop(options);
                Rc::new(Text::new(Theme::italic(&label, enabled), 1, 0, None))
            }
        }
    }
}
/// The message an assistant row reads: the caller's live one or a stored record.
#[derive(Clone)]
enum Source {
    /// Aliases the caller's live message.
    Live(SharedAssistantMessage),
    /// A stored record, read as raw JSON.
    Recorded(Rc<Value>),
}
/// One content block borrowed from either source, reduced to what the row displays.
enum Shown<'a> {
    /// Authored text.
    Text(&'a str),
    /// Authored reasoning.
    Thinking(&'a str),
    /// A tool call, shown by its own row.
    ToolCall,
}
impl<'a> Shown<'a> {
    /// The displayed part of a stored content block; other block types display nothing.
    fn from_record(block: &'a Value) -> Option<Self> {
        let text = |key| block.get(key).and_then(Value::as_str);
        match block.get("type").and_then(Value::as_str)? {
            "text" => text("text").map(Self::Text),
            "thinking" => text("thinking").map(Self::Thinking),
            "toolCall" => Some(Self::ToolCall),
            _ => None,
        }
    }
}
/// How a message ended when it failed.
enum Failure {
    /// The request was aborted.
    Aborted,
    /// The request failed.
    Error,
    /// The model stopped at its output token limit, possibly inside a tool call.
    Length,
}
/// The visible blocks, whether a tool call is present, and the failure notice: shown only
/// without tool calls, except for an output-length stop, which can cut a tool call short.
fn select<'a>(
    content: impl Iterator<Item = Shown<'a>>,
    hide: bool,
    failure: Option<Failure>,
    error: Option<&str>,
) -> (Vec<Visible>, bool, Option<String>) {
    let mut tools = false;
    let mut blocks = Vec::new();
    // The trimmed, non-empty thoughts of the consecutive thinking blocks read so far.
    let mut run: Option<Vec<&str>> = None;
    for block in content {
        let text = match block {
            Shown::Thinking(text) => {
                let thought = text.trim_matches(is_whitespace_scalar);
                let thoughts = run.get_or_insert_with(Vec::new);
                if !thought.is_empty() {
                    thoughts.push(thought);
                }
                continue;
            }
            Shown::Text(text) => Some(text),
            Shown::ToolCall => None,
        };
        blocks.extend(run.take().and_then(|thoughts| thinking(&thoughts, hide)));
        match text {
            Some(text) => blocks.extend(Visible::from_content(text, Visible::Text, false)),
            None => tools = true,
        }
    }
    blocks.extend(run.and_then(|thoughts| thinking(&thoughts, hide)));
    let notice = failure
        .filter(|failure| !tools || matches!(failure, Failure::Length))
        .map(|failure| notice(&failure, error));
    (blocks, tools, notice)
}
/// One section for a run of consecutive thinking blocks: their thoughts joined by a blank
/// line, or nothing when every thought was empty.
fn thinking(thoughts: &[&str], hide: bool) -> Option<Visible> {
    Visible::from_content(&thoughts.join("\n\n"), Visible::Thinking, hide)
}
/// Visible Markdown text or a body-free hidden thought; excludes tool calls.
enum Visible {
    /// Ordinary assistant Markdown.
    Text(String),
    /// Visible thinking Markdown.
    Thinking(String),
    /// Hidden thinking whose label is read when building its child.
    HiddenThinking,
}
impl Visible {
    /// Trims authored content before testing visibility; Markdown cleans decoded content.
    /// Hidden nonblank thinking shows only its label.
    fn from_content(text: &str, constructor: fn(String) -> Self, hidden: bool) -> Option<Self> {
        let text = text.trim_matches(is_whitespace_scalar);
        if text.is_empty() {
            None
        } else if hidden {
            Some(Self::HiddenThinking)
        } else {
            Some(constructor(text.to_owned()))
        }
    }
}
/// Selects a failure notice without changing whitespace-only custom errors.
fn notice(failure: &Failure, error: Option<&str>) -> String {
    let error = error.filter(|text| !text.is_empty());
    match failure {
        Failure::Aborted => error
            .filter(|text| *text != "Request was aborted")
            .unwrap_or("Operation aborted")
            .into(),
        Failure::Error => format!("Error: {}", error.unwrap_or("Unknown error")),
        Failure::Length => "Error: Model stopped because it reached the maximum output token limit. The response may be incomplete.".into(),
    }
}
component_children!(AssistantMessageComponent, Container, container);
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
