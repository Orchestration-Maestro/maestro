//! Terminal history and retained handoff state.
use super::{PaneRuntime, PaneRuntimeConfig};
use maestro_process::pty::PtySize;
use maestro_workspace::layout::PaneId;
use std::{io, sync::atomic::Ordering};

/// Replay limit per pane during server replacement.
const MAX_REPLAY_BYTES_PER_PANE: usize = 8 * 1024;

impl PaneRuntime {
    /// Retained primary-screen ANSI history, absent when no content is retained.
    #[must_use]
    pub fn snapshot_history(&self) -> Option<String> {
        let ansi = self.recent_unwrapped_ansi(usize::MAX);
        (!ansi.trim().is_empty()).then_some(ansi)
    }

    /// Recent primary-screen history for handoff; alternate screens are not replayed.
    #[must_use]
    pub fn handoff_history_ansi(&self) -> Option<String> {
        if self.alternate_screen_active() {
            return None;
        }
        self.snapshot_history()
            .map(|history| truncate_handoff_history(history, MAX_REPLAY_BYTES_PER_PANE))
    }
}

/// Retains only complete recent lines, starting at a UTF-8 boundary.
pub(super) fn truncate_handoff_history(history: String, max_bytes: usize) -> String {
    if history.len() <= max_bytes {
        return history;
    }
    let mut start = history.len().saturating_sub(max_bytes);
    while !history.is_char_boundary(start) {
        start += 1;
    }
    let Some(newline) = history[start..].find('\n') else {
        return String::new();
    };
    history[start + newline + 1..].to_owned()
}

impl PaneRuntime {
    /// Captures terminal negotiation and title state without transient client state.
    #[must_use]
    pub fn handoff_runtime_state(
        &self,
        pane_id: u32,
    ) -> crate::handoff_runtime::HandoffRuntimeState {
        let size = self.size.get();
        crate::handoff_runtime::HandoffRuntimeState {
            pane_id,
            child_pid: self.child_pid().unwrap_or(0),
            rows: size.rows,
            cols: size.cols,
            cell_width_px: size.cell_width_px,
            cell_height_px: size.cell_height_px,
            keyboard_protocol_flags: match self.keyboard_protocol() {
                crate::input::KeyboardProtocol::Legacy => 0,
                crate::input::KeyboardProtocol::Kitty { flags } => flags,
            },
            keyboard_protocol_ansi: self.kitty_keyboard_state_ansi(),
            input_state: self.input_state(),
            terminal_title: self.terminal_title(),
            initial_history_ansi: None,
        }
    }
}

impl PaneRuntime {
    /// Imports a received master, starting with its reader paused and ownership external.
    ///
    /// # Errors
    /// Returns terminal creation or native actor setup failures.
    pub fn from_handoff_fd(
        mut config: PaneRuntimeConfig,
        import: crate::handoff_runtime::ImportedHandoffRuntime,
    ) -> io::Result<Self> {
        let state = import.state;
        config.pane_id = PaneId::from_raw(state.pane_id);
        config.size = PtySize {
            rows: state.rows,
            cols: state.cols,
            cell_width_px: state.cell_width_px,
            cell_height_px: state.cell_height_px,
        };
        let terminal = super::launch::make_terminal(&config)?;
        let _ = terminal.resize(
            state.rows,
            state.cols,
            state.cell_width_px,
            state.cell_height_px,
        );
        terminal.seed_terminal_title(state.terminal_title);
        if let Some(input) = state.input_state {
            terminal.seed_handoff_input_state(input);
        }
        if let Some(ansi) = state.keyboard_protocol_ansi {
            terminal.seed_keyboard_protocol_ansi(&ansi);
        } else {
            terminal.seed_keyboard_protocol_flags(state.keyboard_protocol_flags);
        }
        if let Some(history) = state.initial_history_ansi {
            terminal.seed_history_ansi(&history);
        }
        Self::assemble(
            config,
            terminal,
            import.master_fd,
            state.child_pid,
            super::launch::RuntimeOrigin::Imported,
        )
    }

    /// Closes user admission and waits for accepted writes before pausing reads.
    ///
    /// # Errors
    /// Returns the actor's native or handoff-budget error.
    pub async fn pause_handoff_reader(&self, budget: std::time::Duration) -> io::Result<()> {
        self.io.begin_handoff(budget).await
    }

    /// Duplicates an already quiesced master with owned descriptor lifetime.
    ///
    /// # Errors
    /// Returns an invalid-state, duplicate or closed-actor error.
    pub async fn duplicate_handoff_fd(&self) -> io::Result<std::os::fd::OwnedFd> {
        self.io.duplicate_for_handoff().await
    }

    /// Pauses or resumes terminal reads during a handoff transaction.
    ///
    /// # Errors
    /// Returns the actor's quiescence or rollback failure.
    pub async fn set_handoff_reader_paused(&self, paused: bool) -> io::Result<()> {
        if paused {
            self.pause_handoff_reader(std::time::Duration::from_secs(1))
                .await
        } else {
            self.io.rollback_handoff().await
        }
    }

    /// Relinquishes I/O after handoff commit, preserving the transferred process session.
    ///
    /// # Errors
    /// Returns actor release failure; process ownership remains transferred on failure.
    pub async fn preserve_for_handoff(mut self) -> io::Result<()> {
        let released = self.io.release_after_commit().await;
        self.stop_detection().await;
        self.preserve_processes_on_drop = true;
        released
    }

    /// Takes responsibility for terminating the imported process session on close.
    pub fn assume_handoff_ownership(&mut self) {
        self.preserve_processes_on_drop = false;
    }

    /// Requests a temporary geometry change followed by restoration of the current size.
    pub fn nudge_child_redraw_after_handoff(&self) {
        self.io.nudge_child_redraw_after_handoff(self.size.get());
    }

    /// Resizes only on change, preserving the terminal's minimum pane dimensions.
    pub fn resize(&self, rows: u16, cols: u16, cell_width_px: u32, cell_height_px: u32) {
        let size = PtySize {
            rows: rows.max(2),
            cols: cols.max(4),
            cell_width_px,
            cell_height_px,
        };
        if self.size.replace(size) == size {
            return;
        }
        let responses = self.shared.terminal.resize(
            size.rows,
            size.cols,
            size.cell_width_px,
            size.cell_height_px,
        );
        super::super::agent_detection::mark_detection_content_changed(
            &self.shared.detection_content_seq,
        );
        self.io.resize(size, responses);
    }

    /// Current terminal rows and columns.
    #[must_use]
    pub fn current_size(&self) -> (u16, u16) {
        let size = self.size.get();
        (size.rows, size.cols)
    }

    /// Even after a completed output transaction and odd while one is in progress.
    #[must_use]
    pub fn content_seq(&self) -> u64 {
        self.shared.content_seq.load(Ordering::Acquire)
    }
}
