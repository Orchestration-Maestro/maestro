//! Mouse reporting for terminal applications.

pub mod size;

use num_traits::ToPrimitive;

use super::{
    key_mods::Mods,
    mouse::{MouseAction, MouseButton, MouseEvent},
};
use crate::{
    mouse::{Event, Format},
    point::Coordinate,
};
use size::{Coordinate as Position, Size, Space};

/// Mouse reporting options and optional last-cell tracking.
#[derive(Clone, Debug)]
pub struct MouseEncoder {
    /// Activities the application requested.
    event: Event,
    /// Wire format requested by the application.
    format: Format,
    /// Stable renderer size for position conversion.
    size: Size,
    /// Whether a button is held, including the current event's press.
    pub any_button_pressed: bool,
    /// Outer none disables deduplication; inner none tracks without a prior cell.
    /// Set to `Some(None)` to reset tracking, or `None` to disable it.
    pub last_cell: Option<Option<Coordinate>>,
}

impl MouseEncoder {
    /// Create a disabled encoder for the supplied renderer geometry.
    #[must_use]
    pub fn new(size: Size) -> Self {
        Self {
            event: Event::None,
            format: Format::X10,
            size,
            any_button_pressed: false,
            last_cell: None,
        }
    }

    /// Snapshot terminal reporting modes with caller-supplied renderer geometry.
    /// An explicit format overrides the terminal's format. Button and cell
    /// tracking start disabled and may be configured by the caller.
    #[must_use]
    pub fn from_terminal(terminal: &crate::Terminal, size: Size, format: Option<Format>) -> Self {
        Self {
            event: terminal.flags.mouse_event,
            format: format.unwrap_or(terminal.flags.mouse_format),
            ..Self::new(size)
        }
    }

    /// Current reporting class.
    #[must_use]
    pub fn event(&self) -> Event {
        self.event
    }

    /// Select a reporting class, clearing the tracked cell when it changes.
    pub fn set_event(&mut self, event: Event) {
        if self.event != event {
            self.reset_cell();
            self.event = event;
        }
    }

    /// Current wire format.
    #[must_use]
    pub fn format(&self) -> Format {
        self.format
    }

    /// Select a wire format, clearing the tracked cell when it changes.
    pub fn set_format(&mut self, format: Format) {
        if self.format != format {
            self.reset_cell();
            self.format = format;
        }
    }

    /// Current renderer geometry.
    #[must_use]
    pub fn size(&self) -> Size {
        self.size
    }

    /// Set renderer geometry, clearing the tracked cell even for equal geometry.
    pub fn set_size(&mut self, size: Size) {
        self.size = size;
        self.reset_cell();
    }

    /// Clear a retained cell without enabling or disabling tracking.
    fn reset_cell(&mut self) {
        if let Some(last) = &mut self.last_cell {
            *last = None;
        }
    }

    /// Encode one event, updating enabled cell tracking even if its button or
    /// coordinates cannot be represented in the selected wire format.
    pub fn encode(&mut self, event: &MouseEvent) -> Vec<u8> {
        if !self.should_report(event) || !self.in_viewport(event) {
            return Vec::new();
        }
        let position = Position::Surface {
            x: f64::from(event.pos.x),
            y: f64::from(event.pos.y),
        };
        let Position::Grid { x, y } = position.convert(Space::Grid, self.size) else {
            return Vec::new();
        };
        let cell = Coordinate { x, y: u32::from(y) };
        if event.action == MouseAction::Motion
            && self.format != Format::SgrPixels
            && self.last_cell == Some(Some(cell))
        {
            return Vec::new();
        }
        if let Some(last) = &mut self.last_cell {
            *last = Some(cell);
        }
        let Some(code) = self.button_code(event) else {
            return Vec::new();
        };
        self.sequence(event, position, cell, code)
    }

    /// Apply the selected event class before inspecting positions.
    fn should_report(&self, event: &MouseEvent) -> bool {
        match self.event {
            Event::None => false,
            Event::X10 => {
                event.action == MouseAction::Press
                    && matches!(
                        event.button,
                        Some(MouseButton::Left | MouseButton::Middle | MouseButton::Right)
                    )
            }
            Event::Normal => event.action != MouseAction::Motion,
            Event::Button => event.button.is_some(),
            Event::Any => true,
        }
    }

    /// Releases always pass; off-surface drags require motion tracking and a button.
    fn in_viewport(&self, event: &MouseEvent) -> bool {
        let pos = event.pos;
        let outside = pos.x < 0.0
            || pos.y < 0.0
            || f64::from(pos.x) > f64::from(self.size.screen.width)
            || f64::from(pos.y) > f64::from(self.size.screen.height);
        event.action == MouseAction::Release
            || !outside
            || (matches!(self.event, Event::Button | Event::Any) && self.any_button_pressed)
    }

    /// Resolve button identity, legacy release identity and modifier bits.
    fn button_code(&self, event: &MouseEvent) -> Option<u8> {
        let legacy_release = event.action == MouseAction::Release
            && !matches!(self.format, Format::Sgr | Format::SgrPixels);
        let mut code = if legacy_release || event.button.is_none() {
            3
        } else {
            match event.button? {
                MouseButton::Left => 0,
                MouseButton::Middle => 1,
                MouseButton::Right => 2,
                MouseButton::Four => 64,
                MouseButton::Five => 65,
                MouseButton::Six => 66,
                MouseButton::Seven => 67,
                MouseButton::Eight => 128,
                MouseButton::Nine => 129,
                MouseButton::Unknown | MouseButton::Ten | MouseButton::Eleven => return None,
            }
        };
        if self.event != Event::X10 {
            for (modifier, bit) in [(Mods::SHIFT, 4), (Mods::ALT, 8), (Mods::CTRL, 16)] {
                code += u8::from(event.mods.contains(modifier)) * bit;
            }
        }
        if event.action == MouseAction::Motion {
            code += 32;
        }
        Some(code)
    }

    /// Serialize the selected protocol using already resolved coordinates.
    fn sequence(
        &self,
        event: &MouseEvent,
        position: Position,
        cell: Coordinate,
        code: u8,
    ) -> Vec<u8> {
        let x = u32::from(cell.x) + 1;
        let y = cell.y + 1;
        let final_byte = if event.action == MouseAction::Release {
            'm'
        } else {
            'M'
        };
        match self.format {
            Format::X10 => {
                let (Ok(x), Ok(y)) = (u8::try_from(x + 32), u8::try_from(y + 32)) else {
                    return Vec::new();
                };
                vec![0x1b, b'[', b'M', 32 + code, x, y]
            }
            Format::Utf8 => {
                let mut bytes = vec![0x1b, b'[', b'M', 32 + code];
                for character in [x + 32, y + 32].into_iter().filter_map(char::from_u32) {
                    bytes.extend_from_slice(character.encode_utf8(&mut [0; 4]).as_bytes());
                }
                bytes
            }
            Format::Sgr => format!("\x1b[<{code};{x};{y}{final_byte}").into_bytes(),
            Format::Urxvt => format!("\x1b[{};{x};{y}M", 32 + code).into_bytes(),
            Format::SgrPixels => {
                let Position::Terminal { x, y } = position.convert(Space::Terminal, self.size)
                else {
                    return Vec::new();
                };
                let x = x.round().to_i32().unwrap_or_default();
                let y = y.round().to_i32().unwrap_or_default();
                format!("\x1b[<{code};{x};{y}{final_byte}").into_bytes()
            }
        }
    }
}
