//! Shell input: extension interception, execution and pending-row movement.

use super::InteractiveMode;
use crate::BashExecutionComponent;
use crate::local_dispatch::local_dispatch;
use maestro_app::ReadonlySessionManager;
use maestro_app::agent_session::{BashChunkCallback, BashRunOptions};
use maestro_app::is_whitespace_scalar;
use maestro_app::{
    AgentSession, BashOperations, BashResult, DiagnosticErrorInfo, Presence, TruncationResult,
    UserBashEvent, UserBashEventResult,
};
use maestro_tui::TUI;
use std::{cell::RefCell, collections::HashSet, error::Error, rc::Rc};

/// The callable operations an extension supplies for one shell command.
pub(super) type ShellOperations = Rc<dyn BashOperations<maestro_app::Local>>;

/// The shell command a submission selected, which an extension cannot rewrite.
#[derive(Clone, Copy)]
struct Selected<'a> {
    /// The command to run.
    command: &'a str,
    /// Whether its output stays out of the model context.
    excluded: bool,
}

/// The shell command `text` selects and whether its output stays out of the context.
///
/// One leading `!` includes the output and two exclude it; a command that is empty after
/// the prefix, or text without a prefix, selects nothing.
pub(super) fn selected_shell_command(text: &str) -> Option<(&str, bool)> {
    let rest = text.strip_prefix('!')?;
    let (rest, exclude_from_context) = match rest.strip_prefix('!') {
        Some(rest) => (rest, true),
        None => (rest, false),
    };
    let command = rest.trim_matches(is_whitespace_scalar);
    (!command.is_empty()).then_some((command, exclude_from_context))
}

/// The address of the component `handle` holds, so two handles to one component share it.
pub(super) fn component_address<T: ?Sized>(handle: &Rc<T>) -> usize {
    Rc::as_ptr(handle).cast::<()>() as usize
}

/// Completes `component` from `result`, keeping only the truncation flag its row reads.
fn complete_bash_component(component: &BashExecutionComponent, result: &BashResult) {
    let truncation = result.truncated.then(|| TruncationResult {
        content: String::new(),
        truncated: true,
        truncated_by: None,
        total_lines: 0,
        total_bytes: 0,
        output_lines: 0,
        output_bytes: 0,
        last_line_partial: false,
        first_line_exceeds_limit: false,
        max_lines: 0,
        max_bytes: 0,
    });
    component.set_complete(
        result.exit_code,
        result.cancelled,
        truncation,
        result.full_output_path.clone(),
    );
}

impl InteractiveMode {
    /// Runs `command`, which an extension may answer whole or give operations for.
    ///
    /// # Errors
    /// Returns the failure of the extension runner, of the row's output cleaning or of the
    /// recorded result.
    pub(super) async fn handle_bash_command(
        self: &Rc<Self>,
        command: &str,
        exclude_from_context: bool,
    ) -> Result<(), Box<dyn Error>> {
        let session = self.runtime_host.session();
        let mut event = UserBashEvent {
            command: command.to_owned(),
            exclude_from_context,
            cwd: session.session_manager().borrow().get_cwd().to_owned(),
        };
        let answer = session
            .extension_runner()
            .emit_user_bash::<ShellOperations, BashResult>(&mut event)
            .await
            .map_err(|error| -> Box<dyn Error> { error })?;
        let selected = Selected {
            command,
            excluded: exclude_from_context,
        };
        let row = self.attach_shell_row(selected.command, selected.excluded);
        // The reference reads the current session after the hook, so a replacement during
        // interception records into the session that is current now.
        let session = self.runtime_host.session();
        let outcome = self
            .settle_shell_command(&session, selected, &row, answer)
            .await;
        self.release_shell_row(&row);
        self.ui.request_render(false);
        outcome
    }

    /// Shows one shell answer: an extension's whole result, or the session's execution.
    async fn settle_shell_command(
        self: &Rc<Self>,
        session: &Rc<AgentSession>,
        selected: Selected<'_>,
        row: &Rc<BashExecutionComponent>,
        answer: Option<UserBashEventResult<ShellOperations, BashResult>>,
    ) -> Result<(), Box<dyn Error>> {
        let (result, operations) = match answer {
            Some(UserBashEventResult { result, operations }) => (result, operations),
            None => (Presence::Missing, None),
        };
        if let Presence::Present(result) = result {
            // An intercepted result never streams, so its whole output is shown here.
            if !result.output.is_empty() {
                row.append_output(&result.output)?;
            }
            complete_bash_component(row, &result);
            return session
                .record_bash_result(selected.command, &result, Some(selected.excluded))
                .map_err(|error| -> Box<dyn Error> { Box::new(error) });
        }
        let options = BashRunOptions {
            exclude_from_context: Some(selected.excluded),
            operations,
        };
        match execute_bash_command(self, selected.command, Rc::clone(row), options).await {
            // Execution streamed every chunk into the row already; joining them again would
            // paint the output twice before completion.
            Ok(result) => {
                complete_bash_component(row, &result);
                Ok(())
            }
            Err(error) => {
                row.set_complete(None, false, None, None);
                Ok(self.show_error(&format!("Bash command failed: {error}"))?)
            }
        }
    }

    /// Adds a shell row to the pending area while a response streams, otherwise to the
    /// transcript, and makes it the row that is still producing output.
    fn attach_shell_row(
        self: &Rc<Self>,
        command: &str,
        exclude_from_context: bool,
    ) -> Rc<BashExecutionComponent> {
        let row = Rc::new(BashExecutionComponent::new(
            command.to_owned(),
            TUI::clone(&self.ui),
            self.themes.theme(),
            exclude_from_context,
        ));
        if self.runtime_host.session().is_streaming() {
            self.pending_messages_container.add_child(row.clone());
            self.pending_shells.borrow_mut().push(Rc::clone(&row));
        } else {
            self.chat_container.add_child(row.clone());
        }
        *self.active_shell.borrow_mut() = Some(Rc::clone(&row));
        row
    }

    /// Gives up `row`'s claim on the active slot, leaving a newer row's claim in place.
    fn release_shell_row(&self, row: &Rc<BashExecutionComponent>) {
        let mut active = self.active_shell.borrow_mut();
        if active
            .as_ref()
            .is_some_and(|current| Rc::ptr_eq(current, row))
        {
            *active = None;
        }
    }

    /// Moves every pending shell row into the transcript in the order it was shown, leaving
    /// the rows between them where they are.
    pub(super) fn flush_pending_bash_components(&self) {
        let pending = std::mem::take(&mut *self.pending_shells.borrow_mut());
        if pending.is_empty() {
            return;
        }
        let identities: HashSet<usize> = pending.iter().map(component_address).collect();
        let mut kept = Vec::new();
        let mut moved = Vec::new();
        for child in self.pending_messages_container.children().borrow().iter() {
            if identities.contains(&component_address(child)) {
                moved.push(Rc::clone(child));
            } else {
                kept.push(Rc::clone(child));
            }
        }
        self.pending_messages_container
            .set_children(Rc::new(RefCell::new(kept)));
        for child in moved {
            self.chat_container.add_child(child);
        }
    }

    /// Runs the shell command `text` selects, keeping the editor's text when a command is
    /// already running.
    pub(super) fn submit_shell_command(
        self: &Rc<Self>,
        text: &str,
        command: &str,
        exclude_from_context: bool,
    ) {
        if self.runtime_host.session().is_bash_running() {
            if let Err(error) = self.current_editor().try_set_text(text) {
                return self.report(Err(error));
            }
            self.report(
                self.show_warning(
                    "A bash command is already running. Press Esc to cancel it first.",
                )
                .map_err(Into::into),
            );
            return;
        }
        if let Err(error) = self.current_editor().try_add_to_history(text) {
            return self.report(Err(error));
        }
        let mode = Rc::clone(self);
        let command = command.to_owned();
        self.tool_environment
            .runtime
            .spawn_local(Box::pin(async move {
                let outcome = mode
                    .handle_bash_command(&command, exclude_from_context)
                    .await;
                mode.is_bash_mode.set(false);
                let border = mode.update_editor_border_color();
                outcome?;
                Ok(border?)
            }));
    }
}

/// Runs `command` through `options`, delivering every chunk to `row` before it completes.
///
/// Queued chunks reach the row before command completion. A row whose update fails ends
/// delivery and reports through the runtime's error route.
///
/// # Errors
/// Returns the shell failure the session reports.
async fn execute_bash_command(
    mode: &Rc<InteractiveMode>,
    command: &str,
    row: Rc<BashExecutionComponent>,
    options: BashRunOptions,
) -> Result<BashResult, DiagnosticErrorInfo> {
    let ui = mode.ui.clone();
    let dispatch = local_dispatch(&mode.tool_environment.runtime, move |chunk: String| {
        row.append_output(&chunk)?;
        ui.request_render(false);
        Ok(())
    });
    let mut send = dispatch.sender();
    let on_chunk: BashChunkCallback = Box::new(move |chunk: &str| send(chunk.to_owned()));
    let session = mode.runtime_host.session();
    let result = session.execute_bash(command, Some(on_chunk), options).await;
    // A chunk the row could not show ends the command like the reference's thrown update.
    dispatch.settle().await?;
    result
}
