//! Retained raw bytes with bounded host reply and discarded-tail state.

use super::framing::{
    ControlString, ControlStringFamily, control_string, control_string_terminator_for_family,
};
use super::framing::{
    ESC, PASTE_END, PASTE_START, find_subsequence, host_frame_length,
    starts_with_incomplete_utf8_char,
};
use super::tails::{
    discard_complete_orphaned_sgr_mouse_tail, discard_host_reply_csi_tail,
    discard_or_buffer_orphaned_sgr_mouse_tail, discard_orphaned_sgr_mouse_tail, host_cell_size,
    is_color_scheme_report, is_host_color_response, plausible_control_string_tail,
    starts_with_incomplete_default_color_response, starts_with_incomplete_default_mouse_sequence,
    starts_with_incomplete_host_cell_size_report, starts_with_incomplete_host_color_scheme_report,
    starts_with_incomplete_orphaned_sgr_mouse_tail, starts_with_incomplete_sgr_mouse_sequence,
};
use crate::keys::parse_terminal_key_sequence;

/// Owned pending host bytes and chat text with independent framing policies.
///
/// No clock or reader is owned here. Host callers select a 10 ms idle flush,
/// or 150 ms only while an incomplete mouse frame is pending.
#[derive(Debug, Default)]
pub struct RawInputByteFramer {
    /// Pending binary host input, never decoded through chat text.
    buffer: Vec<u8>,
    /// Pending decoded chat text; binary host input never enters it.
    pub(super) chat_buffer: String,
    /// Termination rule for a timed-out control tail.
    discard_until: Option<ControlStringFamily>,
    /// Bytes already discarded within the bounded tail window.
    discarded_tail_bytes: usize,
    /// Whether an orphaned SGR tail may follow the last Escape.
    lone_escape_recently_flushed: bool,
    /// Outstanding default and palette colour replies.
    host_color_replies_awaited: u16,
    /// Outstanding cell-size replies.
    host_cell_size_replies_awaited: u16,
    /// Whether a focus-triggered appearance reply is outstanding.
    host_appearance_reply_awaited: bool,
    /// Whether the one-extra-flush reply window was consumed.
    held_pending_host_reply_esc: bool,
    /// Opt-in host behavior, independent of chat state.
    policy: HostInputPolicy,
}

/// Opt-in host query behavior and doubled-Escape admission.
#[derive(Debug, Default)]
struct HostInputPolicy {
    /// Rearm colour queries after an appearance report.
    host_color_scheme_change_tracking: bool,
    /// Expect an appearance reply after focus gain.
    host_appearance_query_on_focus: bool,
    /// Separate coalesced Escapes on non-macOS hosts.
    split_coalesced_escape: bool,
}

/// Two default colours plus all 256 palette entries.
const HOST_COLOR_QUERY_REPLIES: u16 = 258;
/// One cell-size reply per query.
const HOST_CELL_SIZE_QUERY_REPLIES: u16 = 1;
/// Maximum complete orphaned mouse tail recognized after a lone Escape.
pub(super) const MAX_ORPHANED_SGR_MOUSE_TAIL_BYTES: usize = 32;

impl RawInputByteFramer {
    /// Select the native host doubled-Escape policy.
    #[must_use]
    pub fn for_host_input() -> Self {
        Self::with_host_input_policy(cfg!(target_os = "macos"))
    }

    /// Select whether doubled Escape remains one legacy Alt sequence.
    #[must_use]
    pub fn with_host_input_policy(preserve_legacy_doubled_escape_input: bool) -> Self {
        Self {
            policy: HostInputPolicy {
                split_coalesced_escape: !preserve_legacy_doubled_escape_input,
                ..Default::default()
            },
            ..Self::default()
        }
    }

    /// Append owned bytes and return complete original frames in order.
    pub fn push(&mut self, data: &[u8]) -> Vec<Vec<u8>> {
        self.buffer.extend_from_slice(data);
        self.drain_available_chunks()
    }

    /// Hold a lone trailing ESC for one idle flush so an OSC 10/11 reply split
    /// at its ESC introducer stitches back together instead of leaking.
    pub fn host_color_query_sent(&mut self) {
        self.host_color_replies_awaited = HOST_COLOR_QUERY_REPLIES;
        self.held_pending_host_reply_esc = false;
    }

    /// Arm one appearance reply window.
    fn host_appearance_query_sent(&mut self) {
        self.host_appearance_reply_awaited = true;
        self.held_pending_host_reply_esc = false;
    }

    /// Same hold window as `host_color_query_sent`, for the XTWINOPS cell size
    /// reply.
    pub fn host_cell_size_query_sent(&mut self) {
        self.host_cell_size_replies_awaited = HOST_CELL_SIZE_QUERY_REPLIES;
        self.held_pending_host_reply_esc = false;
    }

    /// Whether any bounded host query window is armed.
    fn awaiting_host_reply(&self) -> bool {
        self.host_color_replies_awaited > 0
            || self.host_cell_size_replies_awaited > 0
            || self.host_appearance_reply_awaited
    }

    /// Rearm colour reply windows after appearance reports.
    pub fn enable_host_color_scheme_change_tracking(&mut self) {
        self.policy.host_color_scheme_change_tracking = true;
    }

    /// Arm the bounded host-reply window when focus gain will emit an appearance query.
    /// If the write or reply fails, a lone Escape is delayed for only one extra flush.
    pub fn enable_host_appearance_query_on_focus(&mut self) {
        self.policy.host_appearance_query_on_focus = true;
    }

    /// Whether binary host input awaits framing.
    #[must_use]
    pub fn has_pending_input(&self) -> bool {
        !self.buffer.is_empty()
    }

    /// Whether the pending host input is exactly one Escape.
    #[must_use]
    pub fn has_pending_lone_escape(&self) -> bool {
        self.buffer.as_slice() == [ESC]
    }

    /// Whether the current incomplete frame selects the 150 ms mouse timeout.
    #[must_use]
    pub fn has_pending_incomplete_mouse_sequence(&self) -> bool {
        starts_with_incomplete_sgr_mouse_sequence(&self.buffer)
            || starts_with_incomplete_default_mouse_sequence(&self.buffer)
    }

    /// Whether a paste is waiting for its closing marker.
    #[must_use]
    pub fn has_pending_bracketed_paste(&self) -> bool {
        self.buffer.starts_with(PASTE_START) && find_subsequence(&self.buffer, PASTE_END).is_none()
    }

    /// Settle incomplete host input after the caller-selected idle interval.
    pub fn flush_timeout(&mut self) -> Vec<Vec<u8>> {
        let mut chunks = self.drain_available_chunks();

        if self.flush_discard_state()
            || self.buffer.is_empty()
            || self.flush_mouse_tail()
            || self.hold_paste_or_color()
            || self.flush_csi_reply()
            || self.flush_appearance_reply()
            || self.flush_control_string()
            || self.flush_escape(&mut chunks)
        {
            return chunks;
        }

        if let Ok(text) = std::str::from_utf8(&self.buffer)
            && parse_terminal_key_sequence(text).is_some()
        {
            chunks.push(std::mem::take(&mut self.buffer));
            return chunks;
        }

        if starts_with_incomplete_utf8_char(&self.buffer) {
            tracing::trace!(bytes = ?self.buffer, "waiting for UTF-8 continuation bytes");
            return chunks;
        }

        if self.buffer.first() == Some(&ESC) && starts_with_incomplete_utf8_char(&self.buffer[1..])
        {
            tracing::trace!(bytes = ?self.buffer, "waiting for escaped UTF-8 continuation bytes");
            return chunks;
        }

        tracing::debug!(bytes = ?self.buffer, "dropping incomplete raw input buffer after timeout");
        self.lone_escape_recently_flushed = false;
        self.buffer.clear();
        chunks
    }

    /// Settle a discarded tail at quiescence without producing input.
    fn flush_discard_state(&mut self) -> bool {
        if let Some(family) = self.discard_until {
            if family == ControlStringFamily::HostReplyCsi {
                return true;
            }
            if family == ControlStringFamily::OrphanedSgrMouseTail {
                self.buffer.clear();
                self.discard_until = None;
                self.discarded_tail_bytes = 0;
                return true;
            }

            let keep_split_st = self.buffer.last() == Some(&ESC);
            let keep_discarding = plausible_control_string_tail(family, &self.buffer);
            self.discarded_tail_bytes = self.discarded_tail_bytes.saturating_add(self.buffer.len());
            self.buffer.clear();
            if keep_discarding && self.discarded_tail_bytes <= MAX_DISCARDED_CONTROL_TAIL_BYTES {
                self.buffer.extend(keep_split_st.then_some(ESC));
            } else {
                self.discard_until = None;
                self.discarded_tail_bytes = 0;
            }
            return true;
        }

        false
    }

    /// Discard an incomplete mouse frame or orphaned tail after timeout.
    fn flush_mouse_tail(&mut self) -> bool {
        if self.lone_escape_recently_flushed && self.buffer.starts_with(b"[<") {
            tracing::debug!(
                len = self.buffer.len(),
                "discarding incomplete orphaned SGR mouse tail after input timeout"
            );
            discard_or_buffer_orphaned_sgr_mouse_tail(
                &mut self.buffer,
                &mut self.discard_until,
                &mut self.discarded_tail_bytes,
            );
            self.lone_escape_recently_flushed = false;
            return true;
        }

        if starts_with_incomplete_sgr_mouse_sequence(&self.buffer) {
            tracing::debug!(
                bytes = ?self.buffer,
                "discarding incomplete SGR mouse sequence after input timeout"
            );
            self.discarded_tail_bytes = self.buffer.len();
            self.discard_until = (self.discarded_tail_bytes <= MAX_DISCARDED_CONTROL_TAIL_BYTES)
                .then_some(ControlStringFamily::OrphanedSgrMouseTail);
            self.buffer.clear();
            return true;
        }

        false
    }

    /// Retain open pastes and incomplete default-colour replies.
    fn hold_paste_or_color(&mut self) -> bool {
        if self.buffer.starts_with(PASTE_START)
            && find_subsequence(&self.buffer, PASTE_END).is_none()
        {
            tracing::trace!(
                len = self.buffer.len(),
                "waiting for bracketed paste terminator"
            );
            return true;
        }

        if starts_with_incomplete_default_color_response(&self.buffer) {
            tracing::trace!(
                len = self.buffer.len(),
                "waiting for host color response terminator"
            );
            return true;
        }

        false
    }

    /// Give an expected CSI reply one extra flush, then discard its bounded tail.
    fn flush_csi_reply(&mut self) -> bool {
        if (self.host_cell_size_replies_awaited > 0 || self.host_appearance_reply_awaited)
            && self.buffer.as_slice() == b"\x1b["
        {
            if !self.held_pending_host_reply_esc {
                self.held_pending_host_reply_esc = true;
                tracing::trace!("holding incomplete host CSI reply one flush");
                return true;
            }
            self.host_cell_size_replies_awaited = 0;
            self.host_appearance_reply_awaited = false;
            self.held_pending_host_reply_esc = false;
        }

        if self.host_cell_size_replies_awaited > 0
            && starts_with_incomplete_host_cell_size_report(&self.buffer)
        {
            tracing::debug!(
                len = self.buffer.len(),
                "discarding incomplete host cell size report after input timeout"
            );
            self.host_cell_size_replies_awaited = 0;
            self.held_pending_host_reply_esc = false;
            self.discard_until = Some(ControlStringFamily::HostReplyCsi);
            self.discarded_tail_bytes = 0;
            self.buffer.clear();
            return true;
        }

        false
    }

    /// Hold or discard an incomplete appearance report independently of colours.
    fn flush_appearance_reply(&mut self) -> bool {
        if starts_with_incomplete_host_color_scheme_report(&self.buffer) {
            if self.host_appearance_reply_awaited && !self.held_pending_host_reply_esc {
                self.held_pending_host_reply_esc = true;
                tracing::trace!(
                    len = self.buffer.len(),
                    "holding incomplete host color scheme report one flush"
                );
                return true;
            }
            tracing::debug!(
                len = self.buffer.len(),
                "discarding incomplete host color scheme report after input timeout"
            );
            self.host_appearance_reply_awaited = false;
            self.held_pending_host_reply_esc = false;
            self.discard_until = Some(ControlStringFamily::HostReplyCsi);
            self.discarded_tail_bytes = 0;
            self.buffer.clear();
            return true;
        }

        false
    }

    /// Prevent a timed-out control string from leaking as Alt input.
    fn flush_control_string(&mut self) -> bool {
        if let Some(ControlString::Incomplete { family }) = control_string(&self.buffer) {
            tracing::debug!(
                len = self.buffer.len(),
                "discarding incomplete host control string after input timeout"
            );
            // This intentionally gives host control replies precedence over legacy
            // Alt forms like Alt+] after timeout, so later reply tails cannot leak.
            self.discard_until = Some(family);
            self.discarded_tail_bytes = 0;
            self.buffer.clear();
            return true;
        }

        false
    }

    /// Hold a queried Escape once, otherwise emit its original byte.
    fn flush_escape(&mut self, chunks: &mut Vec<Vec<u8>>) -> bool {
        if self.buffer.as_slice() == [ESC] {
            if self.awaiting_host_reply() && !self.held_pending_host_reply_esc {
                self.held_pending_host_reply_esc = true;
                tracing::trace!("holding lone escape one flush while awaiting host reply");
                return true;
            }
            // No continuation arrived; give up the window so Escape is not delayed again.
            self.host_color_replies_awaited = 0;
            self.host_cell_size_replies_awaited = 0;
            self.host_appearance_reply_awaited = false;
            self.held_pending_host_reply_esc = false;
            tracing::warn!(
                bytes = ?self.buffer,
                "flushing lone escape after input timeout; if this follows an alt chord or focus switch it may reach the pane as plain esc"
            );
            self.lone_escape_recently_flushed = true;
            chunks.push(std::mem::take(&mut self.buffer));
            return true;
        }

        false
    }

    /// Consume complete frames in order while retaining the incomplete suffix.
    fn drain_available_chunks(&mut self) -> Vec<Vec<u8>> {
        let mut chunks = Vec::new();

        loop {
            if self.lone_escape_recently_flushed
                && starts_with_incomplete_orphaned_sgr_mouse_tail(&self.buffer)
            {
                break;
            }
            if self.lone_escape_recently_flushed
                && discard_complete_orphaned_sgr_mouse_tail(&mut self.buffer)
            {
                self.lone_escape_recently_flushed = false;
                continue;
            }
            self.lone_escape_recently_flushed = false;
            if self
                .discard_until
                .is_some_and(|family| !self.drain_discarded_tail(family))
            {
                break;
            }
            if self.discard_until.take().is_some() {
                self.discarded_tail_bytes = 0;
                continue;
            }

            if self.policy.split_coalesced_escape && self.buffer.starts_with(b"\x1b\x1b") {
                chunks.push(vec![ESC]);
                self.buffer.drain(..1);
                continue;
            }

            let Some(consumed) = host_frame_length(&self.buffer) else {
                break;
            };
            self.observe_reply(consumed);
            chunks.push(self.buffer[..consumed].to_vec());
            self.buffer.drain(..consumed);
        }

        chunks
    }
    /// Consume one discarded tail, reporting whether ordinary framing can resume.
    fn drain_discarded_tail(&mut self, family: ControlStringFamily) -> bool {
        match family {
            ControlStringFamily::HostReplyCsi => {
                discard_host_reply_csi_tail(&mut self.buffer, &mut self.discarded_tail_bytes)
            }
            ControlStringFamily::OrphanedSgrMouseTail => {
                discard_orphaned_sgr_mouse_tail(&mut self.buffer, &mut self.discarded_tail_bytes)
            }
            _ => {
                let Some(length) = control_string_terminator_for_family(&self.buffer, family)
                else {
                    return false;
                };
                self.buffer.drain(..length);
                true
            }
        }
    }

    /// Settle or rearm the source-specific host query windows after a complete frame.
    fn observe_reply(&mut self, consumed: usize) {
        let frame = &self.buffer[..consumed];
        if is_host_color_response(frame) {
            self.host_color_replies_awaited = self.host_color_replies_awaited.saturating_sub(1);
        } else if host_cell_size(frame).is_some() {
            self.host_cell_size_replies_awaited =
                self.host_cell_size_replies_awaited.saturating_sub(1);
        } else if self.policy.host_appearance_query_on_focus && frame == b"\x1b[I" {
            self.host_appearance_query_sent();
        } else if is_color_scheme_report(frame) {
            self.host_appearance_reply_awaited = false;
            if self.policy.host_color_scheme_change_tracking {
                self.host_color_query_sent();
            }
        }
        self.held_pending_host_reply_esc = false;
    }
}

/// Maximum bytes consumed while discarding a plausible timed-out tail.
pub(super) const MAX_DISCARDED_CONTROL_TAIL_BYTES: usize = 128;

#[cfg(test)]
mod tests;
