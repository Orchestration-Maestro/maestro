//! The expansion and thinking shortcuts, and the editor border that follows the mode.

use super::{InteractiveMode, startup::ExpandableText};
use crate::components::thinking_selector::thinking_text;
use crate::{
    BashExecutionComponent, BranchSummaryMessageComponent, CompactionSummaryMessageComponent,
    CustomMessageComponent, SkillInvocationMessageComponent, ToolExecutionComponent,
};
use maestro_app::agent_session::CycleDirection;
use maestro_app::{EventError, Local, SettingsEdit};
use maestro_theme::ThemeError;
use maestro_tui::Component;
use std::{any::Any, error::Error, rc::Rc};

/// A mode operation an editor shortcut runs.
type Operation = fn(&Rc<InteractiveMode>) -> Result<(), Box<dyn Error>>;

impl InteractiveMode {
    /// Binds the transcript shortcuts and editor actions on the editor, in the order the
    /// editor consults them, then installs its change and image-paste callbacks.
    pub(super) fn setup_key_handlers(self: &Rc<Self>) {
        self.on_action("app.clear", |mode| {
            mode.handle_ctrl_c();
            Ok(())
        });
        self.on_action("app.exit", |mode| {
            mode.handle_ctrl_d();
            Ok(())
        });
        #[cfg(unix)]
        self.on_action("app.suspend", |mode| mode.handle_ctrl_z());
        #[cfg(windows)]
        self.on_action("app.suspend", |mode| mode.show_suspend_unsupported());
        self.on_action("app.thinking.cycle", |mode| mode.cycle_thinking_level());
        self.on_model_cycle("app.model.cycleForward", CycleDirection::Forward);
        self.on_model_cycle("app.model.cycleBackward", CycleDirection::Backward);
        self.on_action("app.session.tree", |mode| mode.show_tree_selector(None));
        self.on_action("app.session.fork", Self::show_user_message_selector);
        self.on_action("app.session.resume", Self::show_session_selector);
        self.on_action("app.model.select", |mode| mode.show_model_selector(None));
        self.on_action("app.tools.expand", |mode| {
            mode.toggle_tool_output_expansion()
        });
        self.on_action("app.thinking.toggle", |mode| {
            mode.toggle_thinking_block_visibility()
        });
        self.on_action("app.editor.external", |mode| mode.open_external_editor());
        self.on_action("app.session.new", |mode| {
            let work = Box::pin(Rc::clone(mode).handle_clear_command());
            mode.tool_environment.runtime.spawn_local(work);
            Ok(())
        });
        self.setup_editor_callbacks();
    }

    /// Binds `action` to `operation` without retaining the mode; a failure goes to the
    /// terminal runtime's error route.
    fn on_action(self: &Rc<Self>, action: &str, operation: Operation) {
        let mode = Rc::downgrade(self);
        self.editor.on_action(
            action.to_owned(),
            Rc::new(move || {
                if let Some(mode) = mode.upgrade() {
                    mode.report(operation(&mode));
                }
            }),
        );
    }

    /// Flips the expansion of tool output and summaries.
    ///
    /// # Errors
    /// Returns the failure of a tool row's native formatting or of a text rendering.
    pub(super) fn toggle_tool_output_expansion(&self) -> Result<(), Box<dyn Error>> {
        self.set_tools_expanded(!self.tool_output_expanded.get())
            .map_err(|error| -> Box<dyn Error> { error })
    }

    /// Applies `expanded` to the active header and every expandable chat row, walking
    /// the child array held when the walk starts, then requests a frame.
    ///
    /// # Errors
    /// Returns the failure of the extension header's expansion, of a tool row's native
    /// formatting or of a text rendering.
    pub(super) fn set_tools_expanded(&self, expanded: bool) -> Result<(), EventError> {
        self.tool_output_expanded.set(expanded);
        let custom = self.custom_header.borrow().clone();
        if let Some(custom) = custom {
            if let Some(set_expanded) = &custom.set_expanded {
                set_expanded.call(expanded)?;
            }
        } else if let Some(header) = self.built_in_header.borrow().clone() {
            set_expanded(header.as_ref(), expanded)?;
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
        self.current_editor().set_border_color(color);
        self.ui.request_render(false);
        Ok(())
    }

    /// Flips and saves whether thinking is hidden, replays the transcript with it, puts the
    /// streaming row back after it with the new visibility and shows the new state.
    ///
    /// # Errors
    /// Returns the failure of the replay or of the published theme.
    fn toggle_thinking_block_visibility(&self) -> Result<(), Box<dyn Error>> {
        let hide = !self.hide_thinking_block.get();
        let session = self.runtime_host.session();
        let admitted = session
            .settings_manager()
            .borrow()
            .admit_settings_edit(SettingsEdit::new(&["hideThinkingBlock"], hide.into()));
        let edit = match admitted {
            Ok(edit) => edit,
            Err(message) => return self.show_error(&message).map_err(Into::into),
        };
        self.hide_thinking_block.set(hide);
        session
            .settings_manager()
            .borrow_mut()
            .apply_prepared_edit(edit);
        self.rebuild_chat_from_messages()
            .map_err(|error| -> Box<dyn Error> { error })?;
        let streaming = self.streaming.borrow().clone();
        if let Some((component, message)) = streaming {
            component.set_hide_thinking_block(hide);
            component.update_content(message);
            self.chat_container.add_child(component);
        }
        let state = if hide { "hidden" } else { "visible" };
        self.show_status(&format!("Thinking blocks: {state}"))
            .map_err(Into::into)
    }
}

/// Expands or collapses `child` when it is one of the expandable rows.
///
/// # Errors
/// Returns the failure of a tool row's native formatting or of a text rendering.
pub(super) fn set_expanded(child: &dyn Component, expanded: bool) -> Result<(), EventError> {
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
