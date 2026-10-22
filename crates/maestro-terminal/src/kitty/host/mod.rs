//! Host terminal graphics cache and its wire encoding.
//!
//! The host cache remembers which images the host terminal already holds and
//! where they are shown. A host image identity hashes the raw 32-bit pane id with
//! the payload signature; a host placement identity hashes the source with the
//! protocol image and placement ids. Every rectangle here is a cell rectangle the
//! caller fills in: nothing in this module reads a workspace, an application or a
//! rendering library.

use crate::kitty::graphics::placements::KittyImagePlacement;

mod cache;

mod cell_size;

mod clip;

mod encode;

mod identity;

mod update;

#[cfg(test)]
mod tests;

pub use cache::HostGraphicsCache;

pub use cell_size::{HostCellSize, WindowSize};

pub use clip::clipped_placement;

pub use encode::{DirectFileCommand, direct_file_command};

#[cfg(unix)]
pub use encode::encode_kitty_regular_file;

/// Cell rectangle in the terminal grid.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    /// Leftmost column.
    pub x: u16,
    /// Topmost row.
    pub y: u16,
    /// Width in columns.
    pub width: u16,
    /// Height in rows.
    pub height: u16,
}

impl Rect {
    /// Rectangle at a position with a size.
    #[must_use]
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// Opaque identity of the view whose placements the host currently shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HostView(u64);

impl HostView {
    /// Identity the caller derives from its own view key.
    #[must_use]
    pub const fn new(key: u64) -> Self {
        Self(key)
    }
}

/// One source whose pixels the host terminal holds.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum HostSourceKey {
    /// An image the terminal screen itself carries.
    Terminal {
        /// Pane showing the image.
        pane_id: u32,
        /// Protocol image identity inside the pane.
        image_id: u32,
    },
    /// A pane graphics layer drawn over the cells.
    PaneLayer {
        /// Pane showing the layer.
        pane_id: u32,
        /// Layer identity inside the pane.
        layer_id: String,
    },
}

impl HostSourceKey {
    /// Sort key that keeps terminal sources ahead of pane layers within a pane.
    fn order(&self) -> (u32, &str) {
        match self {
            Self::Terminal { pane_id, .. } => (*pane_id, ""),
            Self::PaneLayer { pane_id, layer_id } => (*pane_id, layer_id.as_str()),
        }
    }
}

/// Visible placement together with the area and cell size it is drawn in.
#[derive(Clone, Debug)]
pub struct HostPlacement {
    /// Pane showing the placement.
    pub pane_id: u32,
    /// Host identity already reserved for the pixels, when the caller keeps one.
    pub host_image_id: Option<u32>,
    /// Pane area in cells.
    pub area: Rect,
    /// Host cell size in pixels.
    pub cell_size: HostCellSize,
    /// Source the pixels belong to.
    pub source_key: HostSourceKey,
    /// Placement the terminal screen reports.
    pub placement: KittyImagePlacement,
    /// Rows scrolled above the bottom of the terminal history.
    pub scrollback_offset: u32,
}

/// Uploaded pixel identity: what the host must already hold for a source.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct ImageSignature {
    /// Image width in pixels.
    pub image_width: u32,
    /// Image height in pixels.
    pub image_height: u32,
    /// Kitty format code.
    pub format_code: u32,
    /// Full payload length.
    pub data_len: usize,
    /// Fingerprint of the payload.
    pub data_fingerprint: u64,
}

/// Shown geometry: where the host currently draws a placement.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct PlacementSignature {
    /// First shown column.
    x: u16,
    /// First shown row.
    y: u16,
    /// Shown columns.
    cols: u32,
    /// Shown rows.
    rows: u32,
    /// Source left pixel.
    source_x: u32,
    /// Source top pixel.
    source_y: u32,
    /// Source width in pixels.
    source_width: u32,
    /// Source height in pixels.
    source_height: u32,
    /// Horizontal in-cell destination offset.
    x_offset: u32,
    /// Vertical in-cell destination offset.
    y_offset: u32,
    /// Stacking order.
    z: i32,
    /// Rows scrolled above the bottom of the terminal history.
    scrollback_offset: u32,
}

/// Placement geometry after clipping to the visible pane area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClippedPlacement {
    /// First shown column.
    x: u16,
    /// First shown row.
    y: u16,
    /// Shown columns.
    cols: u32,
    /// Shown rows.
    rows: u32,
    /// Source left pixel.
    source_x: u32,
    /// Source top pixel.
    source_y: u32,
    /// Source width in pixels.
    source_width: u32,
    /// Source height in pixels.
    source_height: u32,
    /// Horizontal in-cell destination offset.
    x_offset: u32,
    /// Vertical in-cell destination offset.
    y_offset: u32,
}

/// Wire bytes of one update, and whether more updates remain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodedGraphics {
    /// Commands to write to the host terminal.
    pub bytes: Vec<u8>,
    /// Whether the caller must run another update before the frame is complete.
    pub incomplete: bool,
}
