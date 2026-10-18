//! Terminal emulation for workspace panes.

pub mod unicode;

pub mod kitty;

pub mod color;
pub mod page;
pub mod style;
pub use page::Cell;

pub mod sgr;
pub use style::Style;
