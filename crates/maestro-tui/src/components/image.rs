//! Retained inline image component.
use crate::{
    Component,
    images::terminal_image::{
        ImageDimensions, ImageProtocol, ImageRenderOptions, TerminalImage, calculate_image_rows,
        get_image_dimensions, image_fallback,
    },
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
    id: Option<u32>,
    /// Viewport width and owned lines from the last render.
    cache: Option<(usize, Vec<String>)>,
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
            id: options.image_id,
            options,
            terminal: terminal_image,
            cache: None,
        }
    }
    /// Returns the supplied or first allocated Kitty ID, if any.
    #[must_use]
    pub fn get_image_id(&self) -> Option<u32> {
        self.id
    }
    /// Encodes usable geometry before reserving lines; invalid geometry never allocates an ID.
    fn image_lines(&mut self, width: usize) -> Option<Vec<String>> {
        let protocol = self.terminal.get_capabilities().images?;
        let available = width.checked_sub(2)?;
        let target = u32::try_from(available)
            .unwrap_or(u32::MAX)
            .min(self.options.max_width_cells.unwrap_or(60));
        calculate_image_rows(
            self.dimensions,
            target,
            Some(self.terminal.get_cell_dimensions()),
        )?;
        if protocol == ImageProtocol::Kitty && self.id.is_none() {
            self.id = Some(self.terminal.allocate_image_id());
        }
        let result = self.terminal.render_image(
            &self.base64_data,
            self.dimensions,
            ImageRenderOptions {
                max_width_cells: Some(target),
                image_id: self.id,
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
    fn render(&mut self, width: usize) -> Vec<String> {
        if let Some((cached_width, lines)) = &self.cache
            && *cached_width == width
        {
            return lines.clone();
        }
        let lines = self.image_lines(width).unwrap_or_else(|| {
            let fallback = image_fallback(
                &self.mime_type,
                Some(self.dimensions),
                self.options.filename.as_deref(),
            );
            vec![(self.theme.fallback_color)(&fallback)]
        });
        self.cache = Some((width, lines.clone()));
        lines
    }
    fn invalidate(&mut self) {
        self.cache = None;
    }
}
