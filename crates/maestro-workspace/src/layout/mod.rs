//! Binary split layout with retained focus history.
mod geometry;
mod navigation;
mod tile_layout;
mod tree;
pub use geometry::{Borders, Rect, ranges_overlap};
use maestro_protocol::schema::SplitDirection;
pub use navigation::find_in_direction;
pub use tile_layout::{
    NavDirection, Node, PaneId, PaneInfo, SplitBorder, SplitRequest, TileLayout,
};
