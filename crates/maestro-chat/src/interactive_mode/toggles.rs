//! The expansion and thinking shortcuts, and the editor border that follows the mode.

use super::{InteractiveMode, startup::ExpandableText};
use crate::components::thinking_selector::thinking_text;
use crate::{
    BashExecutionComponent, BranchSummaryMessageComponent, CompactionSummaryMessageComponent,
    CustomMessageComponent, SkillInvocationMessageComponent, ToolExecutionComponent,
};
use maestro_app::Local;
use maestro_theme::ThemeError;
use maestro_tui::{Component, EditorComponent};
use std::{any::Any, error::Error, rc::Rc};

/// A mode operation an editor shortcut runs.
type Operation = fn(&InteractiveMode) -> Result<(), Box<dyn Error>>;

impl InteractiveMode {
    /// Binds the transcript shortcuts on the editor.
    pub(super) fn setup_key_handlers(self: &Rc<Self>) {
        self.on_action("app.tools.expand", Self::toggle_tool_output_expansion);
        self.on_action(
            "app.thinking.toggle",
            Self::toggle_thinking_block_visibility,
        );
    }

    /// Binds `action` to `operation` without retaining the mode; a failure goes to the
    /// terminal runtime's error route.
    fn on_action(self: &Rc<Self>, action: &str, operation: Operation) {
        let mode = Rc::downgrade(self);
        let runtime = Rc::clone(&self.tool_environment.runtime);
        self.editor.on_action(
            action.to_owned(),
            Rc::new(move || {
                if let Some(mode) = mode.upgrade()
                    && let Err(error) = operation(&mode)
                {
                    runtime.spawn_local(Box::pin(std::future::ready(Err(error))));
                }
            }),
        );
    }

    /// Flips the expansion of tool output and summaries.
    ///
    /// # Errors
    /// Returns the failure of a tool row's native formatting or of a text rendering.
    fn toggle_tool_output_expansion(&self) -> Result<(), Box<dyn Error>> {
        self.set_tools_expanded(!self.tool_output_expanded.get())
    }

    /// Applies `expanded` to the startup help and every expandable chat row, walking
    /// the child array held when the walk starts, then requests a frame.
    ///
    /// # Errors
    /// Returns the failure of a tool row's native formatting or of a text rendering.
    pub(super) fn set_tools_expanded(&self, expanded: bool) -> Result<(), Box<dyn Error>> {
        self.tool_output_expanded.set(expanded);
        let header = self.built_in_header.borrow().clone();
        if let Some(header) = header {
            header.set_expanded(expanded)?;
        }
        for child in self.chat_container.walk() {
            set_expanded(child.as_ref(), expanded)?;
        }
        self.ui.request_render(false);
        Ok(())
    }
}

impl InteractiveMode {
    /// Colours the editor border for shell input or the session's thinking level, then
    /// requests a frame.
    ///
    /// # Errors
    /// Returns the failure of reading the published theme.
    pub(super) fn update_editor_border_color(&self) -> Result<(), ThemeError> {
        let theme = self.themes.theme().get()?;
        let color = if self.is_bash_mode.get() {
            theme.get_bash_mode_border_color()
        } else {
            let level = self.runtime_host.session().thinking_level();
            theme.get_thinking_border_color(thinking_text(&level).0)
        };
        self.editor.set_border_color(color);
        self.ui.request_render(false);
        Ok(())
    }

    /// Flips and saves whether thinking is hidden, replays the transcript with it and shows
    /// the new state.
    ///
    /// # Errors
    /// Returns the failure of the replay or of the published theme.
    fn toggle_thinking_block_visibility(&self) -> Result<(), Box<dyn Error>> {
        let hide = !self.hide_thinking_block.get();
        self.hide_thinking_block.set(hide);
        let session = self.runtime_host.session();
        session
            .settings_manager()
            .borrow_mut()
            .set_hide_thinking_block(hide);
        self.rebuild_chat_from_messages()?;
        let state = if hide { "hidden" } else { "visible" };
        self.show_status(&format!("Thinking blocks: {state}"))
    }
}

/// Expands or collapses `child` when it is one of the expandable chat rows.
///
/// # Errors
/// Returns the failure of a tool row's native formatting or of a text rendering.
fn set_expanded(child: &dyn Component, expanded: bool) -> Result<(), Box<dyn Error>> {
    let child: &dyn Any = child;
    if let Some(row) = child.downcast_ref::<ToolExecutionComponent>() {
        row.set_expanded(expanded)?;
    } else if let Some(row) = child.downcast_ref::<ExpandableText>() {
        row.set_expanded(expanded)?;
    } else if let Some(row) = child.downcast_ref::<BashExecutionComponent>() {
        row.set_expanded(expanded);
    } else if let Some(row) = child.downcast_ref::<BranchSummaryMessageComponent>() {
        row.set_expanded(expanded);
    } else if let Some(row) = child.downcast_ref::<CompactionSummaryMessageComponent>() {
        row.set_expanded(expanded);
    } else if let Some(row) = child.downcast_ref::<SkillInvocationMessageComponent>() {
        row.set_expanded(expanded);
    } else if let Some(row) =
        child.downcast_ref::<CustomMessageComponent<serde_json::Value, Local>>()
    {
        row.set_expanded(expanded);
    }
    Ok(())
}
