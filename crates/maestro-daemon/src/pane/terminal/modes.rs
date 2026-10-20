//! Input-mode observations from the engine's single live state.
use super::PaneTerminal;
use crate::{
    input::{KeyboardProtocol, MouseProtocolEncoding, MouseProtocolMode},
    pane::WheelRouting,
};
use maestro_terminal::{Mode, Terminal, screen_set::Key};

/// Input modes carried in a pane handoff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InputState {
    /// Whether the alternate screen is selected.
    pub alternate_screen: InputMode,
    /// Cursor keys use application sequences.
    pub application_cursor: InputMode,
    /// Pasted text is bracketed.
    pub bracketed_paste: InputMode,
    /// Focus transitions are reported.
    pub focus_reporting: InputMode,
    /// Mouse events requested by the child.
    pub mouse_protocol_mode: MouseProtocolMode,
    /// Mouse report coordinates and framing.
    pub mouse_protocol_encoding: MouseProtocolEncoding,
    /// Wheel events become cursor keys on the alternate screen.
    pub mouse_alternate_scroll: InputMode,
    /// Extended modifier-key reporting is enabled.
    #[serde(default)]
    pub modify_other_keys: InputMode,
    /// Host colour-scheme changes are reported.
    #[serde(default)]
    pub color_scheme_reporting: InputMode,
}

impl InputState {
    /// Whether the child requests mouse reports.
    #[must_use]
    pub fn mouse_reporting_enabled(self) -> bool {
        self.mouse_protocol_mode != MouseProtocolMode::None
    }

    /// Shell-like applications keep plain page keys in host scrollback.
    #[must_use]
    pub fn plain_page_keys_use_host_scrollback(self) -> bool {
        !bool::from(self.alternate_screen)
            && !self.mouse_reporting_enabled()
            && (!bool::from(self.application_cursor) || bool::from(self.bracketed_paste))
    }
}

impl PaneTerminal {
    /// Read all handoff input modes in one coherent observation.
    #[must_use]
    pub fn input_state(&self) -> Option<InputState> {
        Some(input_state(&self.core.lock().ok()?.terminal))
    }

    /// Resolve the live keyboard protocol, retaining the caller fallback on lock failure.
    #[must_use]
    pub fn keyboard_protocol(&self, fallback: KeyboardProtocol) -> KeyboardProtocol {
        self.core.lock().map_or(fallback, |core| {
            KeyboardProtocol::from_kitty_flags(u16::from(
                core.terminal.kitty_keyboard_flags().int(),
            ))
        })
    }

    /// Whether the child requests reporting of ordinary keys.
    #[must_use]
    pub fn keyboard_report_all_requested(&self) -> bool {
        self.core.lock().is_ok_and(|core| {
            let protocol = KeyboardProtocol::from_kitty_flags(u16::from(
                core.terminal.kitty_keyboard_flags().int(),
            ));
            protocol.reports_all_keys()
                || protocol.reports_event_types() && core.terminal.modify_other_keys_enabled()
        })
    }

    /// Replay the engine's keyboard stack without a second byte tracker.
    #[must_use]
    pub fn kitty_keyboard_state_ansi(&self) -> Option<String> {
        self.core
            .lock()
            .ok()?
            .terminal
            .kitty_keyboard_state_ansi()
            .ok()
            .filter(|text| !text.is_empty())
    }

    /// Whether paste delimiters are enabled.
    #[must_use]
    pub fn bracketed_paste_enabled(&self) -> bool {
        self.mode_enabled(Mode::BracketedPaste)
    }
    /// Whether focus transitions are enabled.
    #[must_use]
    pub fn focus_reporting_enabled(&self) -> bool {
        self.mode_enabled(Mode::FocusEvent)
    }
    /// Whether any mouse tracking mode is enabled.
    #[must_use]
    pub fn mouse_reporting_enabled(&self) -> bool {
        self.core
            .lock()
            .is_ok_and(|core| core.terminal.mouse_tracking_enabled())
    }
    /// Whether mouse coordinates use pixels.
    #[must_use]
    pub fn sgr_pixel_mouse_enabled(&self) -> bool {
        self.mode_enabled(Mode::MouseFormatSgrPixels)
    }
    /// Whether synchronized output suppresses intermediate renders.
    #[must_use]
    pub fn synchronized_output_active(&self) -> bool {
        self.mode_enabled(Mode::SynchronizedOutput)
    }
    /// Whether the alternate screen is selected.
    #[must_use]
    pub fn alternate_screen_active(&self) -> bool {
        self.core
            .lock()
            .is_ok_and(|core| core.terminal.active_screen() == Key::Alternate)
    }
    /// Whether plain page keys should navigate host history.
    #[must_use]
    pub fn plain_page_keys_use_host_scrollback(&self) -> Option<bool> {
        let core = self.core.lock().ok()?;
        let t = &core.terminal;
        Some(
            t.active_screen() != Key::Alternate
                && !t.mouse_tracking_enabled()
                && (!t.modes.get(Mode::CursorKeys) || t.modes.get(Mode::BracketedPaste)),
        )
    }
    /// Resolve wheel handling without taking an aggregate handoff snapshot.
    #[must_use]
    pub fn wheel_routing(&self) -> Option<WheelRouting> {
        let core = self.core.lock().ok()?;
        let t = &core.terminal;
        Some(if t.mouse_tracking_enabled() {
            WheelRouting::MouseReport
        } else if t.active_screen() == Key::Alternate && t.modes.get(Mode::MouseAlternateScroll) {
            WheelRouting::AlternateScroll
        } else {
            WheelRouting::HostScroll
        })
    }
    /// Read one mode without assembling unrelated observations.
    fn mode_enabled(&self, mode: Mode) -> bool {
        self.core
            .lock()
            .is_ok_and(|core| core.terminal.modes.get(mode))
    }
}

/// Translate selected live modes into the shared handoff vocabulary.
fn input_state(t: &Terminal) -> InputState {
    InputState {
        alternate_screen: InputMode::from(t.active_screen() == Key::Alternate),
        application_cursor: InputMode::from(t.modes.get(Mode::CursorKeys)),
        bracketed_paste: InputMode::from(t.modes.get(Mode::BracketedPaste)),
        focus_reporting: InputMode::from(t.modes.get(Mode::FocusEvent)),
        mouse_protocol_mode: mouse_mode(t),
        mouse_protocol_encoding: if t.modes.get(Mode::MouseFormatSgrPixels) {
            MouseProtocolEncoding::SgrPixels
        } else if t.modes.get(Mode::MouseFormatSgr) {
            MouseProtocolEncoding::Sgr
        } else if t.modes.get(Mode::MouseFormatUtf8) {
            MouseProtocolEncoding::Utf8
        } else {
            MouseProtocolEncoding::Default
        },
        mouse_alternate_scroll: InputMode::from(t.modes.get(Mode::MouseAlternateScroll)),
        modify_other_keys: InputMode::from(t.modify_other_keys_enabled()),
        color_scheme_reporting: InputMode::from(t.modes.get(Mode::ReportColorScheme)),
    }
}

/// Apply the source's precedence when several tracking bits are enabled.
fn mouse_mode(t: &Terminal) -> MouseProtocolMode {
    if t.modes.get(Mode::MouseEventAny) {
        MouseProtocolMode::AnyMotion
    } else if t.modes.get(Mode::MouseEventButton) {
        MouseProtocolMode::ButtonMotion
    } else if t.modes.get(Mode::MouseEventNormal) {
        MouseProtocolMode::PressRelease
    } else if t.modes.get(Mode::MouseEventX10) {
        MouseProtocolMode::Press
    } else {
        MouseProtocolMode::None
    }
}

/// Whether one input mode is enabled, serialized as a boolean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(from = "bool", into = "bool")]
pub enum InputMode {
    /// Mode is disabled.
    #[default]
    Disabled,
    /// Mode is enabled.
    Enabled,
}
impl From<bool> for InputMode {
    fn from(enabled: bool) -> Self {
        if enabled {
            Self::Enabled
        } else {
            Self::Disabled
        }
    }
}
impl From<InputMode> for bool {
    fn from(mode: InputMode) -> Self {
        mode == InputMode::Enabled
    }
}
