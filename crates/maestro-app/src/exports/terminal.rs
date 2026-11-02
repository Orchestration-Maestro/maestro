//! Terminal emulator records exposed to the pane renderer.

/// Emulator state, viewport cells and colors used by the multiplexer.
pub mod terminal {
    #[doc(inline)]
    pub use maestro_terminal::{
        Terminal,
        color::{Palette, Rgb},
        kitty::graphics::unicode::PLACEHOLDER,
        page::cell::{Content, Wide},
        render::{Dirty, RenderCell, RenderRow, RenderState},
        style::{Color, Flags},
    };
}
