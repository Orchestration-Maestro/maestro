#![doc = include_str!("../../../../docs/terminal/widgets.md")]

mod background;
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
