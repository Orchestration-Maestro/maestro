#![doc = include_str!("../../../../../docs/terminal/images.md")]

mod capabilities;
mod dimensions;
mod protocol;
pub use dimensions::{
    calculate_image_rows, get_gif_dimensions, get_image_dimensions, get_jpeg_dimensions,
    get_png_dimensions, get_webp_dimensions,
};
pub use protocol::{
    ITerm2Options, ImageSize, KittyOptions, delete_all_kitty_images, delete_kitty_image,
    encode_i_term2, encode_kitty, hyperlink, image_fallback, is_image_line,
};

use std::{cell::RefCell, rc::Rc};

/// Image transport supported by a terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageProtocol {
    /// Kitty graphics transport.
    Kitty,
    /// iTerm2 inline transport.
    Iterm2,
}

/// Detected rendering support.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TerminalCapabilities {
    /// Inline image transport, when supported.
    pub images: Option<ImageProtocol>,
    /// Whether full RGB color is supported.
    pub true_color: bool,
    /// Whether OSC 8 links are supported.
    pub hyperlinks: bool,
}

/// One terminal's shared capability state and replaceable input sources.
#[derive(Clone)]
pub struct TerminalImage {
    /// Cached terminal measurements and support.
    state: Rc<RefCell<State>>,
    /// Current environment lookup, invoked without a state borrow.
    environment: Rc<Environment>,
    /// Image ID source, invoked without a state borrow.
    image_ids: Rc<dyn Fn() -> u32>,
}

/// Cached support for one terminal.
#[derive(Default)]
struct State {
    /// Explicit or lazily detected capabilities.
    capabilities: Option<TerminalCapabilities>,
    /// Cell pixel measurements.
    cells: CellDimensions,
}

impl TerminalImage {
    /// Creates independent state with supplied environment and ID sources.
    /// The ID source must produce values in `1..=0xffff_fffe`.
    pub fn new(
        environment: impl Fn(&str) -> Option<String> + 'static,
        image_ids: impl Fn() -> u32 + 'static,
    ) -> Self {
        Self {
            state: Rc::new(RefCell::new(State::default())),
            environment: Rc::new(environment),
            image_ids: Rc::new(image_ids),
        }
    }
}

/// Pixel size of one terminal cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellDimensions {
    /// Cell width in pixels.
    pub width_px: u32,
    /// Cell height in pixels.
    pub height_px: u32,
}
impl Default for CellDimensions {
    fn default() -> Self {
        Self {
            width_px: 9,
            height_px: 18,
        }
    }
}
impl Default for TerminalImage {
    fn default() -> Self {
        Self::new(native_environment, || rand::random_range(1..=0xffff_fffe))
    }
}
/// Reads native environment values; browser terminals supply their own lookup.
fn native_environment(key: &str) -> Option<String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var_os(key).map(|value| value.to_string_lossy().into_owned())
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = key;
        None
    }
}
impl TerminalImage {
    /// Returns cached support, detecting it lazily without an outstanding borrow.
    #[must_use]
    pub fn get_capabilities(&self) -> TerminalCapabilities {
        let cached = self.state.borrow().capabilities;
        if let Some(caps) = cached {
            return caps;
        }
        let caps = self.detect_capabilities();
        self.state.borrow_mut().capabilities = Some(caps);
        caps
    }
    /// Clears support detection without changing cell measurements.
    pub fn reset_capabilities_cache(&self) {
        self.state.borrow_mut().capabilities = None;
    }
    /// Overrides cached support without changing cell measurements.
    pub fn set_capabilities(&self, caps: TerminalCapabilities) {
        self.state.borrow_mut().capabilities = Some(caps);
    }
    /// Returns current cell measurements.
    #[must_use]
    pub fn get_cell_dimensions(&self) -> CellDimensions {
        self.state.borrow().cells
    }
    /// Updates both measurements only when both are positive.
    pub fn set_cell_dimensions(&self, dims: CellDimensions) {
        if dims.width_px > 0 && dims.height_px > 0 {
            self.state.borrow_mut().cells = dims;
        }
    }
    /// Requests an ID in the supplied source's inclusive `1..=0xffff_fffe` range.
    #[must_use]
    pub fn allocate_image_id(&self) -> u32 {
        (self.image_ids)()
    }
}

/// Image pixel geometry read from a header or supplied by a caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageDimensions {
    /// Pixel width.
    pub width_px: u32,
    /// Pixel height.
    pub height_px: u32,
}

/// Image placement shared by both inline protocols.
#[derive(Clone, Copy, Debug)]
pub struct ImageRenderOptions {
    /// Target width in cells; absent means 80.
    pub max_width_cells: Option<u32>,
    /// Whether iTerm2 preserves aspect ratio.
    pub preserve_aspect_ratio: bool,
    /// Supplied Kitty ID; direct rendering never allocates one.
    pub image_id: Option<u32>,
    /// Whether Kitty applies its default cursor motion.
    pub move_cursor: bool,
}
impl Default for ImageRenderOptions {
    fn default() -> Self {
        Self {
            max_width_cells: None,
            preserve_aspect_ratio: true,
            image_id: None,
            move_cursor: true,
        }
    }
}
/// Encoded placement and its reserved row count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderedImage {
    /// Opaque terminal escape bytes.
    pub sequence: String,
    /// Number of rows reserved by the image.
    pub rows: usize,
    /// Supplied Kitty ID, absent for iTerm2.
    pub image_id: Option<u32>,
}
impl TerminalImage {
    /// Selects the cached protocol and encodes a placement, or returns absence.
    #[must_use]
    pub fn render_image(
        &self,
        base64_data: &str,
        dimensions: ImageDimensions,
        options: ImageRenderOptions,
    ) -> Option<RenderedImage> {
        let protocol = self.get_capabilities().images?;
        let width = options.max_width_cells.unwrap_or(80);
        let rows = calculate_image_rows(dimensions, width, Some(self.get_cell_dimensions()))?;
        let (sequence, image_id) = match protocol {
            ImageProtocol::Kitty => (
                encode_kitty(
                    base64_data,
                    KittyOptions {
                        columns: Some(width),
                        rows: Some(rows),
                        image_id: options.image_id,
                        move_cursor: options.move_cursor,
                    },
                ),
                options.image_id,
            ),
            ImageProtocol::Iterm2 => (
                encode_i_term2(
                    base64_data,
                    ITerm2Options {
                        width: Some(ImageSize::Cells(width)),
                        height: Some(ImageSize::Text("auto".to_owned())),
                        preserve_aspect_ratio: options.preserve_aspect_ratio,
                        ..ITerm2Options::default()
                    },
                ),
                None,
            ),
        };
        Some(RenderedImage {
            sequence,
            rows,
            image_id,
        })
    }
}

/// Current terminal environment lookup.
type Environment = dyn Fn(&str) -> Option<String>;
