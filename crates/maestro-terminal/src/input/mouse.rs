//! Normalized pointer input supplied by a pane's caller.

use super::key_mods::Mods;

/// Action that triggered a pointer event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MouseAction {
    /// A button was pressed.
    #[default]
    Press,
    /// A button was released.
    Release,
    /// The pointer moved.
    Motion,
}

/// Buttons supported by terminal mouse tracking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    /// Unidentified button.
    Unknown,
    /// Left button.
    Left,
    /// Right button.
    Right,
    /// Middle button.
    Middle,
    /// Wheel up.
    Four,
    /// Wheel down.
    Five,
    /// Wheel left.
    Six,
    /// Wheel right.
    Seven,
    /// Eighth button.
    Eight,
    /// Ninth button.
    Nine,
    /// Tenth button, not representable in mouse reports.
    Ten,
    /// Eleventh button, not representable in mouse reports.
    Eleven,
}

/// Surface-space pixel position, including padding and off-surface positions.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pos {
    /// Horizontal position.
    pub x: f32,
    /// Vertical position.
    pub y: f32,
}

/// An owned pointer observation to encode for a terminal application.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MouseEvent {
    /// Triggering action.
    pub action: MouseAction,
    /// Button, or none for motion without a held button.
    pub button: Option<MouseButton>,
    /// Keyboard modifier observations.
    pub mods: Mods,
    /// Surface-space position in scaled pixels.
    pub pos: Pos,
}

impl Pos {
    /// Convert unsigned pixel coordinates with nearest floating-point rounding.
    #[must_use]
    pub fn from_pixels(x: u32, y: u32) -> Self {
        use num_traits::ToPrimitive;
        Self {
            x: x.to_f32().unwrap_or_default(),
            y: y.to_f32().unwrap_or_default(),
        }
    }
}
