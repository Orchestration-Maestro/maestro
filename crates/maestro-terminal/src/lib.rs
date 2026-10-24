//! Terminal emulation for workspace panes.

pub mod unicode;

pub mod focus;
pub mod input;

pub mod modes;
pub use modes::Mode;

pub mod ansi;
pub mod cursor;
pub mod screen;
pub use screen::{Cursor, CursorStyle, Screen};
pub mod screen_set;
pub use screen_set::ScreenSet;

pub mod kitty;

pub mod color;
pub mod page;
pub mod style;
pub use page::{Cell, Page};

pub mod sgr;
pub use style::Style;

mod bitmap_allocator;

mod ref_counted_set;

mod hash_map;

pub mod size;

pub mod hyperlink;

mod charsets;

pub use charsets::{
    ActiveSlot as CharsetActiveSlot, Charset, Slots as CharsetSlot, table as charset_table,
};

pub mod parse_table;
pub mod parser;

pub mod x11_color;

pub mod apc;
pub mod osc;

pub use parser::{Csi, Dcs, Parser};

pub mod point;
pub use point::{Coordinate, Point};

pub mod page_list;
pub use page_list::{PageList, Pin, Scrollbar};

pub mod mouse;
pub mod terminal;
pub use terminal::{SizeError, Terminal};

mod tabstops;

pub mod csi;

pub mod device_attributes;
pub mod device_status;
pub mod effects;
pub mod size_report;
pub mod stream;

pub mod stream_terminal;

pub mod selection;

/// Text output from terminal-owned pages.
pub mod formatter;

/// Retained incremental viewport rendering.
pub mod render;

#[cfg(test)]
extern crate self as maestro_terminal;

#[cfg(test)]
mod tests;
