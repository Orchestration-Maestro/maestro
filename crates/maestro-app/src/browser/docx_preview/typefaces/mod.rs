//! Font declarations and embedded font resources.
mod fonts;
#[path = "font_table.rs"]
mod part;
pub use fonts::{EmbedFontRef, EmbedFontType, FontDeclaration};
pub use part::FontTablePart;
