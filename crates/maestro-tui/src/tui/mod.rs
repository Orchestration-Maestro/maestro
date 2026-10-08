//! Component capabilities, overlay records and the ordered container.

mod component;
mod container;
mod overlay_types;

pub use crate::text::utils::visible_width;
pub use component::{CURSOR_MARKER, Component, Focusable, InputHandler, is_focusable};
pub use container::{ComponentHandle, Container};
pub use overlay_types::{
    OverlayAnchor, OverlayHandle, OverlayMargin, OverlayMarginValue, OverlayOptions, SizeValue,
};
