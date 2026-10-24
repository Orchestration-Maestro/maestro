//! Live mouse reporting with supplied cell or pixel coordinates.
use super::PaneTerminal;
use crate::input::mouse::Position;
use maestro_protocol::frames::{ClientMouseButton, ClientMouseKind};
use maestro_terminal::{
    Mode, Terminal,
    input::{
        key_mods::Mods,
        mouse_encode::{
            MouseEncoder,
            dimensions::{CellSize, Padding, ScreenSize, Size},
        },
        pointer::{MouseAction, MouseButton, MouseEvent, Pos},
    },
    mouse::Format,
};

impl PaneTerminal {
    /// Encode a button transition or a drag only when the child requests it.
    #[must_use]
    pub fn encode_mouse_button(
        &self,
        kind: ClientMouseKind,
        position: Position,
        modifiers: u8,
    ) -> Option<Vec<u8>> {
        if !matches!(
            kind,
            ClientMouseKind::Down(_) | ClientMouseKind::Up(_) | ClientMouseKind::Drag(_)
        ) {
            return None;
        }
        self.encode_mouse_event(kind, position, modifiers)
    }

    /// Encode unbuttoned movement only in any-motion mode.
    #[must_use]
    pub fn encode_mouse_motion(
        &self,
        kind: ClientMouseKind,
        position: Position,
        modifiers: u8,
    ) -> Option<Vec<u8>> {
        if kind != ClientMouseKind::Moved {
            return None;
        }
        self.encode_mouse_event(kind, position, modifiers)
    }

    /// Encode a wheel direction using the child's current report format.
    #[must_use]
    pub fn encode_mouse_wheel(
        &self,
        kind: ClientMouseKind,
        position: Position,
        modifiers: u8,
    ) -> Option<Vec<u8>> {
        if !matches!(
            kind,
            ClientMouseKind::ScrollUp
                | ClientMouseKind::ScrollDown
                | ClientMouseKind::ScrollLeft
                | ClientMouseKind::ScrollRight
        ) {
            return None;
        }
        self.encode_mouse_event(kind, position, modifiers)
    }

    /// Observe modes and geometry atomically before encoding one report.
    fn encode_mouse_event(
        &self,
        kind: ClientMouseKind,
        position: Position,
        modifiers: u8,
    ) -> Option<Vec<u8>> {
        let core = self.core.lock().ok()?;
        if kind == ClientMouseKind::Moved && !core.terminal.modes.get(Mode::MouseEventAny) {
            return None;
        }
        let (size, pos, format) = geometry(&core.terminal, position)?;
        let mut encoder = MouseEncoder::from_terminal(&core.terminal, size, format);
        let (action, button) = mouse_action(kind);
        let bytes = encoder.encode(&MouseEvent {
            action,
            button,
            mods: Mods::from_int(u16::from(modifiers & 15)),
            pos,
        });
        (!bytes.is_empty()).then_some(bytes)
    }
}

/// Project mouse activities without duplicating wire button codes.
fn mouse_action(kind: ClientMouseKind) -> (MouseAction, Option<MouseButton>) {
    match kind {
        ClientMouseKind::Down(button) => (MouseAction::Press, Some(mouse_button(button))),
        ClientMouseKind::Up(button) => (MouseAction::Release, Some(mouse_button(button))),
        ClientMouseKind::Drag(button) => (MouseAction::Motion, Some(mouse_button(button))),
        ClientMouseKind::Moved => (MouseAction::Motion, None),
        ClientMouseKind::ScrollUp => (MouseAction::Press, Some(MouseButton::Four)),
        ClientMouseKind::ScrollDown => (MouseAction::Press, Some(MouseButton::Five)),
        ClientMouseKind::ScrollLeft => (MouseAction::Press, Some(MouseButton::Six)),
        ClientMouseKind::ScrollRight => (MouseAction::Press, Some(MouseButton::Seven)),
    }
}

/// Move the protocol's button identity into the encoder vocabulary.
fn mouse_button(button: ClientMouseButton) -> MouseButton {
    match button {
        ClientMouseButton::Left => MouseButton::Left,
        ClientMouseButton::Right => MouseButton::Right,
        ClientMouseButton::Middle => MouseButton::Middle,
    }
}

/// Use unit cells for cell reports, requiring real geometry for pixel input.
fn geometry(t: &Terminal, position: Position) -> Option<(Size, Pos, Option<Format>)> {
    let (screen, cell, pos, format) = match position {
        Position::Cell { column, row } => (
            ScreenSize {
                width: u32::from(t.cols()),
                height: u32::from(t.rows()),
            },
            CellSize {
                width: 1,
                height: 1,
            },
            Pos {
                x: f32::from(column),
                y: f32::from(row),
            },
            t.modes
                .get(Mode::MouseFormatSgrPixels)
                .then_some(Format::Sgr),
        ),
        Position::Pixels { x, y } => {
            let cell = CellSize {
                width: t.width_px.checked_div(u32::from(t.cols()))?,
                height: t.height_px.checked_div(u32::from(t.rows()))?,
            };
            if cell.width == 0 || cell.height == 0 {
                return None;
            }
            (
                ScreenSize {
                    width: t.width_px,
                    height: t.height_px,
                },
                cell,
                Pos::from_pixels(x, y),
                None,
            )
        }
    };
    Some((
        Size {
            screen,
            cell,
            padding: Padding::default(),
        },
        pos,
        format,
    ))
}
