//! Transcript rows for each session message and the coalescing status line.

use super::InteractiveMode;
use crate::{
    AssistantMessageComponent, AssistantMessageOptions, BashExecutionComponent,
    BranchSummaryMessageComponent, CompactionSummaryMessageComponent, CustomMessageComponent,
    SkillInvocationMessageComponent, UserMessageComponent,
};
use maestro_app::EventError;
use maestro_app::messages::{BashExecutionMessage, CustomAgentMessages};
use maestro_app::{
    AgentMessage, Local, LocaleNumberFormat, NativeAgentMessage, Presence, SharedAssistantMessage,
    TruncationOptions, UserBlock, UserContent, parse_skill_block, truncate_tail,
};
use maestro_theme::{ThemeColor, ThemeError};
use maestro_tui::{Component, MarkdownTheme, Spacer, TUI, Text};
use std::{rc::Rc, sync::PoisonError};

impl InteractiveMode {
    /// The text blocks of a user message joined without a separator; other roles have none.
    pub(super) fn get_user_message_text(message: &NativeAgentMessage) -> String {
        let AgentMessage::User(user) = message else {
            return String::new();
        };
        let user = user.read().unwrap_or_else(PoisonError::into_inner);
        match &user.content {
            UserContent::Text(text) => text.clone(),
            UserContent::Blocks(blocks) => blocks
                .iter()
                .filter_map(|block| match block {
                    UserBlock::Text(text) => Some(text.text.as_str()),
                    UserBlock::Image(_) => None,
                })
                .collect(),
        }
    }

    /// The published theme's Markdown styles with the configured code-block indent.
    pub(super) fn get_markdown_theme_with_settings(&self) -> Rc<MarkdownTheme> {
        let mut theme = maestro_theme::get_markdown_theme(
            self.themes.theme(),
            Rc::clone(&self.tool_environment.highlighter),
            true,
        );
        let session = self.runtime_host.session();
        theme.code_block_indent = Some(session.settings_manager().borrow().get_code_block_indent());
        Rc::new(theme)
    }

    /// Shows a dim status line, replacing the previous one when it is still the last pair
    /// of transcript rows, then requests a frame.
    ///
    /// # Errors
    /// Returns the failure of the published theme's dim colour.
    pub(super) fn show_status(&self, message: &str) -> Result<(), ThemeError> {
        let text = self.themes.theme().get()?.fg(&ThemeColor::Dim, message)?;
        let last = self.last_status.borrow().clone();
        if let Some((spacer, status)) = last
            && self.ends_with(&[spacer.clone(), status.clone()])
        {
            status.set_text(text);
            self.ui.request_render(false);
            return Ok(());
        }
        let spacer: Rc<dyn Component> = Rc::new(Spacer::new(1));
        let status = Rc::new(Text::new(text, 1, 0, None));
        self.chat_container.add_child(Rc::clone(&spacer));
        self.chat_container.add_child(status.clone());
        self.last_status.replace(Some((spacer, status)));
        self.ui.request_render(false);
        Ok(())
    }

    /// Whether the transcript's last rows are exactly `rows`, compared by identity.
    fn ends_with(&self, rows: &[Rc<dyn Component>]) -> bool {
        let children = self.chat_container.children();
        let children = children.borrow();
        children.len() >= rows.len()
            && children[children.len() - rows.len()..]
                .iter()
                .zip(rows)
                .all(|(child, row)| std::ptr::addr_eq(Rc::as_ptr(child), Rc::as_ptr(row)))
    }

    /// Appends the rows of one message: tool results have none, as their call rows show them.
    /// With `populate_history`, a user message's text joins the editor history.
    ///
    /// # Errors
    /// Returns the failure of a shell row's output cleaning or of the locale's number format.
    pub(super) fn add_message_to_chat(
        &self,
        message: &NativeAgentMessage,
        populate_history: bool,
    ) -> Result<(), EventError> {
        match message {
            AgentMessage::User(_) => {
                self.add_user_message(&Self::get_user_message_text(message), populate_history)?;
            }
            AgentMessage::Assistant(message) => {
                let component = self.assistant_component(message.clone());
                self.chat_container.add_child(component);
            }
            AgentMessage::ToolResult(_) => {}
            AgentMessage::Custom(custom) => {
                // The guard is released before any row construction runs an extension renderer.
                let custom = custom
                    .read()
                    .unwrap_or_else(PoisonError::into_inner)
                    .clone();
                self.add_custom_message(custom)?;
            }
        }
        Ok(())
    }

    /// An assistant row showing `message` with the current thinking visibility.
    pub(super) fn assistant_component(
        &self,
        message: SharedAssistantMessage,
    ) -> Rc<AssistantMessageComponent> {
        Rc::new(AssistantMessageComponent::new(
            Some(message),
            self.assistant_options(),
            self.themes.theme(),
            self.get_markdown_theme_with_settings(),
            self.tool_environment.terminal_image.clone(),
        ))
    }

    /// The assistant rows' current thinking visibility and hidden-thinking label.
    pub(super) fn assistant_options(&self) -> AssistantMessageOptions {
        AssistantMessageOptions {
            hide_thinking_block: self.hide_thinking_block.get(),
            hidden_thinking_label: self.working.hidden_thinking_label.borrow().clone(),
            ..AssistantMessageOptions::default()
        }
    }

    /// Appends a nonempty user text, as a skill block with an optional separate user row or
    /// as one user row, after a spacer when the transcript is not empty.
    ///
    /// # Errors
    /// Returns the current editor's original history failure.
    fn add_user_message(&self, text: &str, populate_history: bool) -> Result<(), EventError> {
        if text.is_empty() {
            return Ok(());
        }
        if !self.chat_container.children().borrow().is_empty() {
            self.chat_container.add_child(Rc::new(Spacer::new(1)));
        }
        let user = |text| {
            Rc::new(UserMessageComponent::new(
                text,
                self.themes.theme(),
                self.get_markdown_theme_with_settings(),
                self.tool_environment.terminal_image.clone(),
            ))
        };
        if let Some(skill) = parse_skill_block(text) {
            let trailing = skill.user_message.clone();
            let component = Rc::new(SkillInvocationMessageComponent::new(
                skill,
                self.themes.theme(),
                self.get_markdown_theme_with_settings(),
                self.tool_environment.terminal_image.clone(),
            ));
            component.set_expanded(self.tool_output_expanded.get());
            self.chat_container.add_child(component);
            if let Some(trailing) = trailing {
                self.chat_container.add_child(user(trailing));
            }
        } else {
            self.chat_container.add_child(user(text.to_owned()));
        }
        if populate_history {
            self.current_editor().try_add_to_history(text)?;
        }
        Ok(())
    }

    /// Appends the rows of a shell run, a displayed custom message or a summary.
    ///
    /// # Errors
    /// Returns the failure of a shell row's output cleaning or of the locale's number format.
    fn add_custom_message(&self, message: CustomAgentMessages) -> Result<(), EventError> {
        let expanded = self.tool_output_expanded.get();
        let theme = self.themes.theme();
        let image = self.tool_environment.terminal_image.clone();
        match message {
            CustomAgentMessages::BashExecution(message) => self.add_bash_execution(message)?,
            CustomAgentMessages::Custom(message) if message.display => {
                let runner = self.runtime_host.session().extension_runner();
                let renderer = runner.get_message_renderer_call(&message.custom_type);
                drop(runner);
                let markdown = self.get_markdown_theme_with_settings();
                let component = CustomMessageComponent::<serde_json::Value, Local>::new(
                    Rc::new(message),
                    renderer,
                    theme,
                    markdown,
                    image,
                );
                component.set_expanded(expanded);
                self.chat_container.add_child(Rc::new(component));
            }
            CustomAgentMessages::Custom(_) => {}
            CustomAgentMessages::BranchSummary(message) => {
                self.chat_container.add_child(Rc::new(Spacer::new(1)));
                let markdown = self.get_markdown_theme_with_settings();
                let component = BranchSummaryMessageComponent::new(message, theme, markdown, image);
                component.set_expanded(expanded);
                self.chat_container.add_child(Rc::new(component));
            }
            CustomAgentMessages::CompactionSummary(message) => {
                self.chat_container.add_child(Rc::new(Spacer::new(1)));
                let numbers = LocaleNumberFormat::new(&|key| std::env::var(key).ok())?;
                let markdown = self.get_markdown_theme_with_settings();
                let component = CompactionSummaryMessageComponent::new(
                    message, theme, markdown, image, &numbers,
                )?;
                component.set_expanded(expanded);
                self.chat_container.add_child(Rc::new(component));
            }
            CustomAgentMessages::Other(never) => match never {},
        }
        Ok(())
    }

    /// Appends a completed shell run with its recorded output, exit and truncation.
    ///
    /// # Errors
    /// Returns the failure of the output's control-sequence cleaning.
    fn add_bash_execution(&self, message: BashExecutionMessage) -> Result<(), EventError> {
        let component = BashExecutionComponent::new(
            message.command,
            TUI::clone(&self.ui),
            self.themes.theme(),
            matches!(message.exclude_from_context, Presence::Present(true)),
        );
        if !message.output.is_empty() {
            component.append_output(&message.output)?;
        }
        // The row reads only the truncation flag of a recorded run.
        let truncation = message.truncated.then(|| {
            let mut result = truncate_tail("", TruncationOptions::default());
            result.truncated = true;
            result
        });
        component.set_complete(
            message.exit_code.present().copied(),
            message.cancelled,
            truncation,
            message.full_output_path.present().cloned(),
        );
        self.chat_container.add_child(Rc::new(component));
        Ok(())
    }
}
