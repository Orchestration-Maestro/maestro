//! Public widget construction and inherited editing operations.
use super::input::{AssistantFields, Input};
use maestro_app::{AssistantMessage, SharedAssistantMessage};
use maestro_chat::{
    AssistantMessageComponent, AssistantMessageOptions, BranchSummaryMessageComponent,
    CompactionSummaryMessageComponent, UserMessageComponent,
};
use maestro_theme::LiveTheme;
use maestro_tui::tui::ChildArray;
use maestro_tui::{Component, MarkdownTheme, TerminalImage};
use std::{
    rc::Rc,
    sync::{Arc, RwLock},
};

/// Retains the concrete public type for its inherited child interface.
pub enum Widget {
    User(UserMessageComponent),
    Assistant(AssistantMessageComponent),
    Branch(BranchSummaryMessageComponent),
    Compaction(CompactionSummaryMessageComponent),
}
/// Prepares a real typed assistant record using harmless fixture defaults.
pub fn message(value: Option<&AssistantFields>) -> SharedAssistantMessage {
    let mut result: AssistantMessage = serde_json::from_value(serde_json::json!({
        "role":"assistant", "content":[], "api":"openai-responses", "provider":"openai", "model":"fixture",
        "usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},
        "stopReason":"stop", "timestamp":0
    })).unwrap();
    if let Some(value) = value {
        edit(&mut result, value);
    }
    Arc::new(RwLock::new(result))
}
/// Changes only the recorded fields on the surviving shared message.
pub fn edit(target: &mut AssistantMessage, value: &AssistantFields) {
    if let Some(content) = &value.content {
        target.content = serde_json::from_value(serde_json::to_value(content).unwrap()).unwrap();
    }
    if let Some(reason) = &value.stop_reason {
        target.stop_reason = serde_json::from_value(reason.clone().into()).unwrap();
    }
    if let Some(error) = &value.error_message {
        target.error_message = Some(error.clone());
    }
}
impl Widget {
    /// Uses the same constructors as application callers.
    pub fn new(
        input: &Input,
        live: LiveTheme,
        markdown: Rc<MarkdownTheme>,
    ) -> (Self, Option<SharedAssistantMessage>) {
        let terminal = TerminalImage::new(|_| None, || 1);
        match input.kind.as_str() {
            "user" => (
                Self::User(UserMessageComponent::new(
                    input.text.clone().unwrap(),
                    live,
                    markdown,
                    terminal,
                )),
                None,
            ),
            "assistant" => {
                let retained = input.message.as_ref().map(|value| message(Some(value)));
                let options = AssistantMessageOptions {
                    hide_thinking_block: input.hide.unwrap_or(false),
                    hidden_thinking_label: input
                        .label
                        .clone()
                        .unwrap_or_else(|| "Thinking...".into()),
                    styles_enabled: input.styles_enabled.unwrap_or(true),
                };
                (
                    Self::Assistant(AssistantMessageComponent::new(
                        retained.clone(),
                        options,
                        live,
                        markdown,
                        terminal,
                    )),
                    retained,
                )
            }
            "branch" => (
                Self::Branch(BranchSummaryMessageComponent::new(
                    maestro_app::BranchSummaryMessage {
                        summary: input.text.clone().unwrap(),
                        from_id: "origin".into(),
                        timestamp: 0.0,
                    },
                    live,
                    markdown,
                    terminal,
                )),
                None,
            ),
            "compaction" => (Self::compaction(input, live, markdown, terminal), None),
            _ => panic!("unrecognized fixture kind"),
        }
    }
    /// Constructs localized compaction through its actual constructor route.
    fn compaction(
        input: &Input,
        live: LiveTheme,
        markdown: Rc<MarkdownTheme>,
        terminal: TerminalImage,
    ) -> Self {
        let count = input.tokens.as_ref().unwrap();
        let count = count
            .as_str()
            .map_or_else(|| count.as_f64().unwrap(), |text| text.parse().unwrap());
        let numbers =
            maestro_app::LocaleNumberFormat::new(&|key| input.environment.get(key).cloned())
                .unwrap();
        Self::Compaction(
            CompactionSummaryMessageComponent::new(
                maestro_app::CompactionSummaryMessage {
                    summary: input.text.clone().unwrap(),
                    tokens_before: count,
                    timestamp: 0.0,
                },
                live,
                markdown,
                terminal,
                &numbers,
            )
            .unwrap(),
        )
    }
    /// Requires the assistant constructor route.
    pub fn assistant(&self) -> &AssistantMessageComponent {
        match self {
            Self::Assistant(value) => value,
            _ => panic!("not an assistant"),
        }
    }
    /// Public inherited array, never a copied list.
    pub fn children(&self) -> ChildArray {
        match self {
            Self::User(value) => value.children(),
            Self::Assistant(value) => value.children(),
            Self::Branch(value) => value.children(),
            Self::Compaction(value) => value.children(),
        }
    }
    /// Replaces the inherited array.
    pub fn clear(&self) {
        match self {
            Self::User(value) => value.clear(),
            Self::Assistant(value) => value.clear(),
            Self::Branch(value) => value.clear(),
            Self::Compaction(value) => value.clear(),
        }
    }
    /// Appends through the inherited component interface.
    pub fn append(&self, text: String) {
        self.children()
            .borrow_mut()
            .push(Rc::new(maestro_tui::Text::new(text, 0, 0, None)));
    }
    /// Rejects expansion on nonsummary cases.
    pub fn expand(&self, expanded: bool) {
        match self {
            Self::Branch(value) => value.set_expanded(expanded),
            Self::Compaction(value) => value.set_expanded(expanded),
            _ => panic!("not a summary"),
        }
    }
}
impl Component for Widget {
    fn render(&self, width: usize) -> Vec<String> {
        match self {
            Self::User(value) => value.render(width),
            Self::Assistant(value) => value.render(width),
            Self::Branch(value) => value.render(width),
            Self::Compaction(value) => value.render(width),
        }
    }
    fn invalidate(&self) {
        match self {
            Self::User(value) => value.invalidate(),
            Self::Assistant(value) => value.invalidate(),
            Self::Branch(value) => value.invalidate(),
            Self::Compaction(value) => value.invalidate(),
        }
    }
}
