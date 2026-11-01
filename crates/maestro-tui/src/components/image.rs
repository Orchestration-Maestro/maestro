//! Retained inline image component.
use std::cell::{Cell, RefCell};

use crate::{
    Component, TruncateOptions,
    images::terminal_image::{
        ImageDimensions, ImageProtocol, ImageRenderOptions, TerminalImage, calculate_image_rows,
        fallback_name, get_image_dimensions, image_fallback,
    },
    truncate_to_width,
};

/// Styling supplied for the text image placeholder.
pub struct ImageTheme {
    /// Styles the complete, untruncated fallback text.
    pub fallback_color: Box<dyn Fn(&str) -> String>,
}
/// Retained image placement and metadata.
#[derive(Clone, Debug, Default)]
pub struct ImageOptions {
    /// Maximum cell width; absent means 60.
    pub max_width_cells: Option<u32>,
    /// Maximum cell height; absent means a box as tall in pixels as the width bound is wide.
    pub max_height_cells: Option<u32>,
    /// Optional filename shown in text fallback.
    pub filename: Option<String>,
    /// Supplied Kitty ID; zero is retained but omitted from protocol parameters.
    pub image_id: Option<u32>,
    /// Explicit geometry overriding header inspection.
    pub dimensions: Option<ImageDimensions>,
}
/// An inline image with width-sensitive cached output and retained Kitty ID.
pub struct Image {
    /// Already encoded image contents.
    base64_data: String,
    /// Exact MIME spelling for inspection and fallback.
    mime_type: String,
    /// Retained explicit, inspected, or default pixel size.
    dimensions: ImageDimensions,
    /// Fallback styling callback.
    theme: ImageTheme,
    /// Placement and metadata supplied at construction.
    options: ImageOptions,
    /// Shared terminal support and cell measurements.
    terminal: TerminalImage,
    /// Supplied or lazily allocated Kitty identifier.
    id: Cell<Option<u32>>,
    /// Viewport width and owned lines from the last render.
    cache: RefCell<Option<(usize, Vec<String>)>>,
}
impl Image {
    /// Retains an image, selecting explicit, header, or 800×600 geometry.
    #[must_use]
    pub fn new(
        base64_data: String,
        mime_type: String,
        theme: ImageTheme,
        options: ImageOptions,
        terminal_image: TerminalImage,
    ) -> Self {
        let dimensions = options
            .dimensions
            .or_else(|| get_image_dimensions(&base64_data, &mime_type))
            .unwrap_or(ImageDimensions {
                width_px: 800,
                height_px: 600,
            });
        Self {
            base64_data,
            mime_type,
            dimensions,
            theme,
            id: Cell::new(options.image_id),
            options,
            terminal: terminal_image,
            cache: RefCell::new(None),
        }
    }
    /// Returns the supplied or first allocated Kitty ID, if any.
    #[must_use]
    pub fn get_image_id(&self) -> Option<u32> {
        self.id.get()
    }
    /// The styled fallback text truncated to `width`: the shortened, possibly linked name, the
    /// MIME type and the pixel size.
    fn fallback_line(&self, width: usize) -> String {
        let hyperlinks = self.terminal.get_capabilities().hyperlinks;
        let name = self
            .options
            .filename
            .as_deref()
            .filter(|name| !name.is_empty())
            .map(|name| fallback_name(name, self.terminal.home().as_deref(), hyperlinks));
        let fallback = image_fallback(&self.mime_type, Some(self.dimensions), name.as_deref());
        truncate_to_width(
            &(self.theme.fallback_color)(&fallback),
            width,
            TruncateOptions::default(),
        )
    }
    /// Encodes usable geometry before reserving lines; invalid geometry never allocates an ID.
    fn image_lines(&self, width: usize) -> Option<Vec<String>> {
        let protocol = self.terminal.get_capabilities().images?;
        let available = width.checked_sub(2)?;
        let target = u32::try_from(available)
            .unwrap_or(u32::MAX)
            .min(self.options.max_width_cells.unwrap_or(60));
        let cells = self.terminal.get_cell_dimensions();
        calculate_image_rows(self.dimensions, target, Some(cells))?;
        let square = (u64::from(target) * u64::from(cells.width_px))
            .div_ceil(u64::from(cells.height_px))
            .max(1);
        let max_height = self
            .options
            .max_height_cells
            .unwrap_or_else(|| u32::try_from(square).unwrap_or(u32::MAX));
        if protocol == ImageProtocol::Kitty && self.id.get().is_none() {
            self.id.set(Some(self.terminal.allocate_image_id()));
        }
        let result = self.terminal.render_image(
            &self.base64_data,
            self.dimensions,
            ImageRenderOptions {
                max_width_cells: Some(target),
                max_height_cells: Some(max_height),
                image_id: self.id.get(),
                move_cursor: false,
                ..ImageRenderOptions::default()
            },
        )?;
        let offset = result.rows - 1;
        let mut lines = vec![String::new(); offset];
        let up = if offset > 0 {
            format!("\x1b[{offset}A")
        } else {
            String::new()
        };
        let down = if offset > 0 && protocol == ImageProtocol::Kitty {
            format!("\x1b[{offset}B")
        } else {
            String::new()
        };
        lines.push(format!("{up}{}{down}", result.sequence));
        Some(lines)
    }
}
impl Component for Image {
    fn render(&self, width: usize) -> Vec<String> {
        if let Some((cached_width, lines)) = &*self.cache.borrow()
            && *cached_width == width
        {
            return lines.clone();
        }
        let lines = self
            .image_lines(width)
            .unwrap_or_else(|| vec![self.fallback_line(width)]);
        *self.cache.borrow_mut() = Some((width, lines.clone()));
        lines
    }
    fn invalidate(&self) {
        self.cache.take();
    }
}
