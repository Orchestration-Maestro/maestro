//! Compaction and retry progress: their status indicators, the retry countdown, the
//! interrupt callbacks they replace and their teardown.

use super::InteractiveMode;
use super::status_indicator::StatusKind;
use crate::CountdownTimer;
use maestro_app::agent_session::CompactionReason;
use maestro_app::messages::CustomAgentMessages;
use maestro_app::messages::create_compaction_summary_message;
use maestro_app::{AgentMessage, CompactionResult};
use maestro_theme::{LiveTheme, ThemeColor, paint};
use maestro_tui::{Component, Loader};
use std::{
    cell::RefCell,
    error::Error,
    rc::Rc,
    sync::{Arc, RwLock},
};

/// The interrupt callback a progress state replaced; its absence is the state's absence.
pub(super) struct PreviousEscape(Option<Rc<dyn Fn()>>);

/// The status indicator and the interrupt callbacks the two progress states replaced.
#[derive(Default)]
pub(super) struct Retained {
    /// The status container's indicator and idle rows.
    pub(super) status: super::status_indicator::StatusArea,
    /// The interrupt callback the running compaction replaced.
    compaction_escape: RefCell<Option<PreviousEscape>>,
    /// The interrupt callback the running retry replaced.
    retry_escape: RefCell<Option<PreviousEscape>>,
}

/// What a compaction end reports.
pub(super) struct CompactionOutcome<'a> {
    /// What started the compaction.
    pub(super) reason: CompactionReason,
    /// The stored result of a successful compaction.
    pub(super) result: Option<&'a CompactionResult>,
    /// Whether the compaction was aborted.
    pub(super) aborted: bool,
    /// The failure message of a failed compaction.
    pub(super) error_message: Option<&'a str>,
    /// Whether the failed prompt is retried, which queues the captured input behind it.
    pub(super) will_retry: bool,
}

/// What a retry start reports.
pub(super) struct RetryStart {
    /// The attempt about to run.
    pub(super) attempt: u32,
    /// The configured attempt limit.
    pub(super) max_attempts: f64,
    /// The wait before the attempt.
    pub(super) delay_ms: f64,
}

/// What a retry end reports.
pub(super) struct RetryEnd<'a> {
    /// Whether a retry succeeded.
    pub(super) success: bool,
    /// The last attempt.
    pub(super) attempt: u32,
    /// The final failure.
    pub(super) final_error: Option<&'a str>,
}

impl InteractiveMode {
    /// Shows the compaction label and makes the interrupt key abort the compaction, leaving
    /// the editor's input, focus and submission untouched.
    ///
    /// # Errors
    /// Returns the failure of the terminal's progress.
    pub(super) fn start_compaction(&self, reason: CompactionReason) -> Result<(), Box<dyn Error>> {
        self.set_terminal_progress(true)?;
        self.progress
            .compaction_escape
            .replace(Some(PreviousEscape(self.editor.on_escape())));
        let host = Rc::clone(&self.runtime_host);
        self.editor
            .set_on_escape(Some(Rc::new(move || host.session().abort_compaction())));
        let loader = self.progress_loader(Some(compaction_label(reason)), ThemeColor::Accent);
        self.show_status_indicator(StatusKind::Compaction, loader, None);
        self.ui.request_render(false);
        Ok(())
    }

    /// Shows what the compaction ended with, restores the interrupt callback and the
    /// terminal's progress, removes the loader and delivers the input it captured.
    ///
    /// # Errors
    /// Returns the failure of the transcript, the theme, the terminal's progress or the
    /// pending rows.
    pub(super) fn end_compaction(
        self: &Rc<Self>,
        outcome: &CompactionOutcome<'_>,
    ) -> Result<(), Box<dyn Error>> {
        self.set_terminal_progress(false)?;
        self.restore_compaction_interrupt();
        self.clear_status_indicator(Some(StatusKind::Compaction));
        self.show_compaction_outcome(outcome)?;
        self.flush_compaction_queue(outcome.will_retry)?;
        self.ui.request_render(false);
        Ok(())
    }

    /// Shows the compaction's own outcome: a cancellation notice, the rebuilt transcript
    /// with its summary, or the failure in the role its cause uses.
    fn show_compaction_outcome(
        &self,
        outcome: &CompactionOutcome<'_>,
    ) -> Result<(), Box<dyn Error>> {
        if outcome.aborted {
            return if outcome.reason == CompactionReason::Manual {
                Ok(self.show_error("Compaction cancelled")?)
            } else {
                self.show_status("Auto-compaction cancelled")
                    .map_err(Into::into)
            };
        }
        if let Some(result) = outcome.result {
            return self.append_compaction_summary(result);
        }
        // An empty supplied message takes the no-notice branch, as an absent one does.
        let Some(message) = outcome.error_message.filter(|text| !text.is_empty()) else {
            return Ok(());
        };
        if outcome.reason == CompactionReason::Manual {
            Ok(self.show_error(message)?)
        } else {
            Ok(self.notice(&ThemeColor::Error, message)?)
        }
    }

    /// Clears the transcript, replays the session and appends one synthetic summary timed
    /// now, then invalidates the footer.
    ///
    /// # Errors
    /// Returns the failure of the replay, of the stored record or of the summary's row.
    fn append_compaction_summary(&self, result: &CompactionResult) -> Result<(), Box<dyn Error>> {
        self.rebuild_chat_from_messages()
            .map_err(|error| -> Box<dyn Error> { error })?;
        let timestamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let summary = create_compaction_summary_message(
            result.summary.clone(),
            result.tokens_before,
            &timestamp,
        );
        let message = AgentMessage::Custom(Arc::new(RwLock::new(
            CustomAgentMessages::CompactionSummary(summary),
        )));
        self.add_message_to_chat(&message, false)
            .map_err(|error| -> Box<dyn Error> { error })?;
        self.footer.invalidate();
        Ok(())
    }

    /// Shows the retry label with its countdown to the next attempt, which the interrupt key
    /// aborts; each tick reads the keys bound then.
    pub(super) fn start_retry(&self, retry: &RetryStart) {
        self.progress
            .retry_escape
            .replace(Some(PreviousEscape(self.editor.on_escape())));
        let host = Rc::clone(&self.runtime_host);
        self.editor
            .set_on_escape(Some(Rc::new(move || host.session().abort_retry())));
        let attempt = retry.attempt;
        let limit = retry.max_attempts;
        // The countdown's own first tick writes the opening label before the row is shown.
        let loader = self.progress_loader(None, ThemeColor::Warning);
        let message = loader.clone();
        let on_tick = Rc::new(move |seconds: f64| {
            message.set_message(retry_label(attempt, limit, seconds));
        });
        // An expired countdown stops itself; this owner keeps it until the retry ends.
        let countdown = CountdownTimer::new(
            retry.delay_ms,
            Some(&self.ui),
            Rc::clone(&self.tool_environment.runtime),
            on_tick,
            Rc::new(|| {}),
        );
        self.show_status_indicator(StatusKind::Retry, loader, Some(countdown));
        self.ui.request_render(false);
    }

    /// Removes the retry progress and restores the replaced interrupt callback, then shows
    /// the final failure when the retries are exhausted.
    ///
    /// # Errors
    /// Returns the failure of the transcript's error notice.
    pub(super) fn end_retry(&self, outcome: &RetryEnd<'_>) -> Result<(), Box<dyn Error>> {
        self.restore_retry_interrupt();
        self.clear_status_indicator(Some(StatusKind::Retry));
        if !outcome.success {
            let failure = outcome.final_error.filter(|text| !text.is_empty());
            let failure = failure.unwrap_or("Unknown error");
            self.show_error(&format!(
                "Retry failed after {} attempts: {failure}",
                outcome.attempt
            ))?;
        }
        self.ui.request_render(false);
        Ok(())
    }

    /// Restores the interrupt callback a retry replaced when a response starts; the
    /// working row then replaces the retry indicator.
    pub(super) fn start_agent_progress(&self) {
        self.restore_retry_interrupt();
    }

    /// Restores the callback the compaction replaced, or the editor's ordinary action when
    /// it had none.
    fn restore_compaction_interrupt(&self) {
        if let Some(previous) = self.progress.compaction_escape.take() {
            self.editor.set_on_escape(previous.0);
        }
    }

    /// Restores the callback the retry replaced, or the editor's ordinary action when it had
    /// none.
    fn restore_retry_interrupt(&self) {
        if let Some(previous) = self.progress.retry_escape.take() {
            self.editor.set_on_escape(previous.0);
        }
    }

    /// Builds a loader in the status container's own colours and sizes; an absent message
    /// is the loader's own, which a countdown replaces on its first tick.
    pub(in crate::interactive_mode) fn progress_loader(
        &self,
        message: Option<String>,
        spinner: ThemeColor,
    ) -> Loader {
        let theme = self.themes.theme();
        Loader::new(
            self.ui.as_ref().clone(),
            colored(theme.clone(), spinner),
            colored(theme, ThemeColor::Muted),
            message,
            None,
        )
    }
}

/// Paints `text` in `role`, leaving it unstyled until a theme is published.
fn colored(theme: LiveTheme, role: ThemeColor) -> Rc<dyn Fn(&str) -> String> {
    Rc::new(move |text| paint(theme.get().ok().as_deref(), &role, text))
}

/// The label of a running compaction, with the interrupt keys in effect when it started.
fn compaction_label(reason: CompactionReason) -> String {
    let hint = format!("({} to cancel)", maestro_app::key_text("app.interrupt"));
    match reason {
        CompactionReason::Manual => format!("Compacting context... {hint}"),
        CompactionReason::Threshold => format!("Auto-compacting... {hint}"),
        CompactionReason::Overflow => {
            format!("Context overflow detected, Auto-compacting... {hint}")
        }
    }
}

/// The retry label: the attempt, its limit, the whole seconds left and the live keys.
fn retry_label(attempt: u32, limit: f64, seconds: f64) -> String {
    format!(
        "Retrying ({attempt}/{}) in {}s... ({} to cancel)",
        ryu_js::Buffer::new().format_finite(limit),
        ryu_js::Buffer::new().format_finite(seconds),
        maestro_app::key_text("app.interrupt")
    )
}
