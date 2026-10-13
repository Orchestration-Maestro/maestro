#![doc = include_str!("../../../../docs/terminal/widgets.md")]

mod background;
mod editing;

use std::cell::{Cell, RefCell};

/// Advances `generation` and then drops `cache`, so a render in progress cannot store its rows.
fn invalidate_cache<T>(generation: &Cell<u64>, cache: &RefCell<Option<T>>) {
    generation.set(generation.get().wrapping_add(1));
    cache.take();
}
mod r#box;
mod image;
mod input;
pub use input::Input;

mod cancellable_loader;
mod loader;
pub use cancellable_loader::CancellableLoader;
mod spacer;
mod text;
mod truncated_text;
pub use image::{Image, ImageOptions, ImageTheme};
pub use loader::{Loader, LoaderIndicatorOptions};

pub use r#box::Box;
pub use spacer::Spacer;
pub use text::Text;
pub use truncated_text::TruncatedText;

/// Command-selection component.
pub mod select_list;
pub use select_list::{
    SelectItem, SelectList, SelectListLayoutOptions, SelectListTheme,
    SelectListTruncatePrimaryContext,
};
/// Searchable settings component.
pub mod settings_list;
pub use settings_list::{SettingItem, SettingsList, SettingsListTheme};

/// Multiline editable terminal text.
pub mod editor;
pub use editor::{Editor, EditorOptions, EditorTheme};
mod cursor;
pub mod markdown;
pub use markdown::{DefaultTextStyle, Markdown, MarkdownOptions, MarkdownTheme, TextDecoration};
