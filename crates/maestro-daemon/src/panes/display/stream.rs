//! Ordered PTY effects over the emulator's synchronous write and drain API.

use super::{PaneCore, PaneTerminal, ProcessBytesResult, colors};
use crate::panes::osc::{
    DefaultColorEvent, contains_scrollback_clear_sequence,
    maybe_filter_primary_screen_scrollback_clear, parse_reported_cwd,
};
use maestro_process::jobs::ForegroundJob;
use maestro_terminal::Mode;
use maestro_workspace::tiling::PaneId;
use std::time::{Duration, Instant};
use std::{borrow::Cow, cell::LazyCell};

impl PaneTerminal {
    /// Process live output, requesting foreground facts only for colour sets or primary-screen ED3.
    /// Replies are returned directly, never re-queued as user input.
    pub fn process_pty_bytes(
        &self,
        pane_id: PaneId,
        shell_pid: u32,
        bytes: &[u8],
        foreground_job: impl FnOnce() -> Option<ForegroundJob>,
    ) -> ProcessBytesResult {
        crate::render_prof::counter("pty.bytes", u64::try_from(bytes.len()).unwrap_or(u64::MAX));
        let Ok(mut core) = self.core.lock() else {
            tracing::error!(
                pane = pane_id.raw(),
                "terminal core lock poisoned in reader"
            );
            return ProcessBytesResult::default();
        };
        let foreground_job = LazyCell::new(foreground_job);
        discard_seed_effects(&mut core);
        observe_evidence(&mut core, pane_id, bytes);
        let title_changed = core.agent_osc_state.observe(bytes);
        let alternate_screen =
            core.terminal.active_screen() == maestro_terminal::screens::Key::Alternate;
        let filtered = if shell_pid > 0 {
            let foreground_job = (!alternate_screen && contains_scrollback_clear_sequence(bytes))
                .then(|| foreground_job.as_ref())
                .flatten();
            maybe_filter_primary_screen_scrollback_clear(bytes, alternate_screen, foreground_job)
        } else {
            Cow::Borrowed(bytes)
        };
        let started = crate::render_prof::timer();
        let terminal_responses = write_ordered(&mut core, &filtered, shell_pid, &foreground_job);
        crate::render_prof::duration_since("pty.terminal_write", started);
        let terminal_bells = core.terminal.take_bell_count();
        let clipboard_writes = core.terminal.take_clipboard_writes();
        let reported_cwd = core
            .terminal
            .take_pwd_changes()
            .into_iter()
            .filter_map(|value| parse_reported_cwd(&value))
            .next_back();
        #[cfg(any(windows, test))]
        let reported_cwd = reported_cwd.or_else(|| core.windows_powershell_current_prompt_cwd());
        if cfg!(windows) {
            let current = core.current_cursor_state();
            core.cursor_settle_state.observe(current, Instant::now());
        }
        let synchronized = core.terminal.modes.get(Mode::SynchronizedOutput);
        let graphics =
            core.kitty_graphics_enabled && filtered.windows(3).any(|window| window == b"\x1b_G");
        let render_delay = core.render_delay_after_pty_write(graphics);
        record_render_request(synchronized, render_delay);
        ProcessBytesResult {
            request_render: !synchronized,
            render_delay,
            terminal_title_changed: title_changed,
            terminal_bells,
            clipboard_writes,
            reported_cwd,
            terminal_responses,
        }
    }

    /// Retain the caller's image-rendering policy for the redraw settle backstop.
    pub fn set_kitty_graphics_enabled(&self, enabled: bool) {
        if let Ok(mut core) = self.core.lock() {
            core.kitty_graphics_enabled = enabled;
        }
    }
}

/// Discard effects produced while restoring historical terminal state.
fn discard_seed_effects(core: &mut PaneCore) {
    core.terminal.take_pwd_changes();
    core.terminal.take_bell_count();
    core.terminal.take_clipboard_writes();
}

/// Log passive evidence without changing retained rendering state.
fn observe_evidence(core: &mut PaneCore, pane_id: PaneId, bytes: &[u8]) {
    core.osc_debug_tracker.observe(bytes);
    for event in core.osc_debug_tracker.drain_pending() {
        tracing::debug!(pane = pane_id.raw(), osc_command = %event.command, osc_payload = ?event.payload, "agent OSC evidence observed");
    }
}

/// A completed custom reply or host-colour policy event.
enum OrderedEvent {
    /// Host-colour observation.
    Color(crate::panes::osc::DefaultColorTrackedEvent),
    /// Capability response absent from the engine's DCS consumer.
    Capability(crate::panes::xtgettcap::XtgettcapResponse),
}

impl OrderedEvent {
    /// Exclusive end offset in the current write.
    fn end_offset(&self) -> usize {
        match self {
            Self::Color(event) => event.end_offset,
            Self::Capability(event) => event.end_offset,
        }
    }
}

/// Write only at policy boundaries, draining engine replies before custom responses.
fn write_ordered(
    core: &mut PaneCore,
    bytes: &[u8],
    shell_pid: u32,
    foreground_job: &LazyCell<Option<ForegroundJob>, impl FnOnce() -> Option<ForegroundJob>>,
) -> Vec<Vec<u8>> {
    core.default_color_event_tracker.observe(bytes);
    core.xtgettcap_query_tracker.observe(bytes);
    let mut events: Vec<_> = core
        .default_color_event_tracker
        .drain_pending()
        .into_iter()
        .map(OrderedEvent::Color)
        .collect();
    events.extend(
        core.xtgettcap_query_tracker
            .drain_pending()
            .into_iter()
            .map(OrderedEvent::Capability),
    );
    events.sort_by_key(OrderedEvent::end_offset);
    let mut written = 0;
    let mut responses = std::mem::take(&mut core.pending_pty_responses);
    for event in events {
        let end = event.end_offset().min(bytes.len());
        core.terminal.write(&bytes[written..end]);
        written = end;
        let mut engine_reply = core.terminal.take_pty_reply();
        let custom = custom_response(core, event, shell_pid, foreground_job, &mut engine_reply);
        if !engine_reply.is_empty() {
            responses.push(engine_reply);
        }
        responses.extend(custom);
    }
    core.terminal.write(&bytes[written..]);
    let mut reply = core.terminal.take_pty_reply();
    if let Some(event) = core.default_color_event_tracker.in_progress_event()
        && matches!(
            event,
            DefaultColorEvent::Query(_) | DefaultColorEvent::PaletteQuery(_)
        )
        && colors::respond(core, event).is_some()
    {
        colors::replace_reply(&mut reply, event);
    }
    if !reply.is_empty() {
        responses.push(reply);
    }
    if !core.child_default_foreground_changed && !core.child_default_background_changed {
        core.transient_default_color_owner_pgid = None;
    }
    responses
}

/// Apply custom policy to one completed request, preserving unrelated engine replies.
fn custom_response(
    core: &mut PaneCore,
    event: OrderedEvent,
    shell_pid: u32,
    foreground_job: &LazyCell<Option<ForegroundJob>, impl FnOnce() -> Option<ForegroundJob>>,
    engine_reply: &mut Vec<u8>,
) -> Option<Vec<u8>> {
    match event {
        OrderedEvent::Color(event) => {
            if matches!(event.event, DefaultColorEvent::Set(_)) && shell_pid > 0 {
                core.transient_default_color_owner_pgid = foreground_job
                    .as_ref()
                    .filter(|job| !job.processes.iter().any(|process| process.pid == shell_pid))
                    .map(|job| job.process_group_id)
                    .or(core.transient_default_color_owner_pgid);
            }
            let response = colors::respond(core, event.event);
            if response.is_some() {
                colors::replace_reply(engine_reply, event.event);
            }
            response
        }
        OrderedEvent::Capability(response) => Some(response.bytes),
    }
}

impl PaneCore {
    /// Keep synchronized batches hidden and use only configured settle intervals.
    pub(super) fn render_delay_after_pty_write(&self, graphics: bool) -> Option<Duration> {
        if self.terminal.modes.get(Mode::SynchronizedOutput) {
            None
        } else if graphics {
            Some(Duration::from_millis(20))
        } else if cfg!(windows) && self.cursor_settle_state.pending() {
            Some(crate::panes::settlement::CURSOR_POSITION_SETTLE)
        } else {
            None
        }
    }
}

/// Record the rendering decisions without installing a background profiler loop.
fn record_render_request(synchronized: bool, delay: Option<Duration>) {
    if synchronized {
        crate::render_prof::event("pty.synchronized_output_suppressed");
    } else {
        crate::render_prof::event("pty.request_render");
    }
    if delay.is_some() {
        crate::render_prof::event("pty.request_render_delayed");
    }
}
