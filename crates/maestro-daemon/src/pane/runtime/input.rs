//! User-input delivery and terminal response ordering.
use super::PaneRuntime;
use crate::input::{KeyboardProtocol, TerminalKey};
use crate::terminal_theme::HostAppearance;
use std::time::Duration;
use tokio::sync::mpsc::error::{SendError, TrySendError};

/// Focus transition forwarded when the terminal requests it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusEvent {
    /// The pane gained focus.
    Gained,
    /// The pane lost focus.
    Lost,
}

impl PaneRuntime {
    /// Waits for user-input capacity without dropping caller bytes.
    ///
    /// # Errors
    /// Returns the original bytes when the actor is closed.
    pub async fn send_bytes(&self, bytes: Vec<u8>) -> Result<(), SendError<Vec<u8>>> {
        self.io.write_user_input(bytes).await
    }

    /// Submits bytes without waiting for capacity.
    ///
    /// # Errors
    /// Preserves full versus closed and returns the original bytes.
    pub fn try_send_bytes(&self, bytes: Vec<u8>) -> Result<(), TrySendError<Vec<u8>>> {
        self.io.try_write_user_input(bytes)
    }

    /// Encodes the live keyboard negotiation with a legacy fallback.
    #[must_use]
    pub fn keyboard_protocol(&self) -> KeyboardProtocol {
        self.shared
            .terminal
            .keyboard_protocol(KeyboardProtocol::Legacy)
    }

    /// Encodes a key using the destination terminal's live modes.
    #[must_use]
    pub fn encode_terminal_key(&self, key: TerminalKey) -> Vec<u8> {
        self.shared
            .terminal
            .encode_terminal_key(key, self.keyboard_protocol())
    }

    /// Encodes vertical wheel input as cursor keys only in alternate-scroll mode.
    #[must_use]
    pub fn encode_alternate_scroll(
        &self,
        kind: maestro_protocol::frames::ClientMouseKind,
    ) -> Option<Vec<u8>> {
        use maestro_protocol::frames::{ClientKeyCode, ClientMouseKind};
        if self.wheel_routing()? != super::super::WheelRouting::AlternateScroll {
            return None;
        }
        let key = match kind {
            ClientMouseKind::ScrollUp => ClientKeyCode::Up,
            ClientMouseKind::ScrollDown => ClientKeyCode::Down,
            _ => return None,
        };
        Some(self.encode_terminal_key(TerminalKey::new(key, 0)))
    }

    /// Sends a paste using the destination's bracketed-paste mode.
    ///
    /// # Errors
    /// Returns the encoded payload if the actor is closed.
    pub async fn send_paste(&self, text: String) -> Result<(), SendError<Vec<u8>>> {
        self.send_bytes(self.paste_payload(text)).await
    }

    /// Submits a paste without waiting for capacity.
    ///
    /// # Errors
    /// Returns the encoded payload with the full or closed error.
    pub fn try_send_paste(&self, text: String) -> Result<(), TrySendError<Vec<u8>>> {
        self.try_send_bytes(self.paste_payload(text))
    }

    /// Frames pasted text only when the child enabled paste delimiters.
    #[must_use]
    pub fn paste_payload(&self, text: String) -> Vec<u8> {
        if self.bracketed_paste_enabled() {
            format!("\x1b[200~{text}\x1b[201~").into_bytes()
        } else {
            text.into_bytes()
        }
    }

    /// Returns false when focus reporting is disabled; otherwise attempts delivery.
    #[must_use]
    pub fn try_send_focus_event(&self, event: FocusEvent) -> bool {
        if !self.focus_reporting_enabled() {
            return false;
        }
        let bytes = match event {
            FocusEvent::Gained => b"\x1b[I",
            FocusEvent::Lost => b"\x1b[O",
        };
        if let Err(error) = self.try_send_bytes(bytes.to_vec()) {
            tracing::warn!(%error, "failed to forward pane focus event");
        }
        true
    }

    /// Retains host colours while preserving the child terminal's dynamic overrides.
    pub fn apply_host_terminal_theme(&self, theme: crate::terminal_theme::TerminalTheme) {
        self.shared.terminal.apply_host_terminal_theme(theme);
    }

    /// Applies host colour changes through the same response lane as terminal reads.
    pub fn apply_host_terminal_appearance(&self, appearance: Option<HostAppearance>) {
        self.io.write_terminal_response(|| {
            self.shared
                .terminal
                .apply_host_terminal_appearance(appearance)
        });
    }

    /// Submits delayed user input, retaining its task for runtime teardown.
    pub fn send_bytes_after(&self, bytes: Vec<u8>, delay: Duration) {
        let io = self.io.clone();
        let task = tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            if let Err(error) = io.write_user_input(bytes).await {
                tracing::warn!(%error, "failed to send delayed pane input");
            }
        });
        self.shared.retain_task(task);
    }
}
