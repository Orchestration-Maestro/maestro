//! Terminal emulation for workspace panes.

pub mod unicode;

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
