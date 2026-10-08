//! Inline image wire formats.
use std::fmt::Write as _;

/// Kitty placement parameters.
#[derive(Clone, Copy, Debug)]
pub struct KittyOptions {
    /// Nonzero width in cells.
    pub columns: Option<u32>,
    /// Nonzero reserved row count.
    pub rows: Option<usize>,
    /// Nonzero image identifier.
    pub image_id: Option<u32>,
    /// Whether the terminal applies its default cursor motion.
    pub move_cursor: bool,
}
impl Default for KittyOptions {
    fn default() -> Self {
        Self {
            columns: None,
            rows: None,
            image_id: None,
            move_cursor: true,
        }
    }
}
/// Frames already encoded ASCII data into Kitty chunks of at most 4096 bytes.
#[must_use]
pub fn encode_kitty(base64_data: &str, options: KittyOptions) -> String {
    let mut params = String::from("a=T,f=100,q=2");
    if !options.move_cursor {
        params.push_str(",C=1");
    }
    if let Some(value) = options.columns.filter(|value| *value > 0) {
        let _ = write!(params, ",c={value}");
    }
    if let Some(value) = options.rows.filter(|value| *value > 0) {
        let _ = write!(params, ",r={value}");
    }
    if let Some(value) = options.image_id.filter(|value| *value > 0) {
        let _ = write!(params, ",i={value}");
    }
    if base64_data.len() <= 4096 {
        return format!("\x1b_G{params};{base64_data}\x1b\\");
    }
    let mut output = String::new();
    let mut chunks = base64_data.as_bytes().chunks(4096).enumerate().peekable();
    while let Some((index, chunk)) = chunks.next() {
        let payload = String::from_utf8_lossy(chunk);
        let continuation = u8::from(chunks.peek().is_some());
        if index == 0 {
            let _ = write!(output, "\x1b_G{params},m=1;{payload}\x1b\\");
        } else {
            let _ = write!(output, "\x1b_Gm={continuation};{payload}\x1b\\");
        }
    }
    output
}

/// An iTerm2 size in cells or verbatim protocol text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImageSize {
    /// Whole terminal cells.
    Cells(u32),
    /// A protocol size such as `auto` or `50%`.
    Text(String),
}
impl std::fmt::Display for ImageSize {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cells(value) => value.fmt(formatter),
            Self::Text(value) => value.fmt(formatter),
        }
    }
}
/// iTerm2 inline placement parameters.
#[derive(Clone, Debug)]
pub struct ITerm2Options {
    /// Width, including an explicitly supplied zero or empty string.
    pub width: Option<ImageSize>,
    /// Height, including an explicitly supplied zero or empty string.
    pub height: Option<ImageSize>,
    /// Optional UTF-8 filename.
    pub name: Option<String>,
    /// Whether to retain the image's aspect ratio.
    pub preserve_aspect_ratio: bool,
    /// Whether to display inline instead of downloading.
    pub inline: bool,
}
impl Default for ITerm2Options {
    fn default() -> Self {
        Self {
            width: None,
            height: None,
            name: None,
            preserve_aspect_ratio: true,
            inline: true,
        }
    }
}
/// Encodes an iTerm2 command without altering the already encoded payload.
#[must_use]
pub fn encode_i_term2(base64_data: &str, options: ITerm2Options) -> String {
    use base64::Engine as _;
    let mut params = format!("inline={}", u8::from(options.inline));
    if let Some(value) = options.width {
        let _ = write!(params, ";width={value}");
    }
    if let Some(value) = options.height {
        let _ = write!(params, ";height={value}");
    }
    if let Some(name) = options.name.filter(|value| !value.is_empty()) {
        let _ = write!(
            params,
            ";name={}",
            base64::prelude::BASE64_STANDARD.encode(name)
        );
    }
    if !options.preserve_aspect_ratio {
        params.push_str(";preserveAspectRatio=0");
    }
    format!("\x1b]1337;File={params}:{base64_data}\x07")
}

/// Formats the untruncated placeholder, including only present metadata.
#[must_use]
pub fn image_fallback(
    mime_type: &str,
    dimensions: Option<super::ImageDimensions>,
    filename: Option<&str>,
) -> String {
    let mut parts = Vec::new();
    if let Some(filename) = filename.filter(|value| !value.is_empty()) {
        parts.push(filename.to_owned());
    }
    parts.push(format!("[{mime_type}]"));
    if let Some(dims) = dimensions {
        parts.push(format!("{}x{}", dims.width_px, dims.height_px));
    }
    format!("[Image: {}]", parts.join(" "))
}

/// Recognizes either inline image prefix anywhere, independently of support detection.
#[must_use]
pub fn is_image_line(line: &str) -> bool {
    line.contains("\x1b_G") || line.contains("\x1b]1337;File=")
}

/// Quietly deletes an image and frees its data.
#[must_use]
pub fn delete_kitty_image(image_id: u32) -> String {
    format!("\x1b_Ga=d,d=I,i={image_id},q=2\x1b\\")
}
/// Quietly deletes every visible image and frees its data.
#[must_use]
pub fn delete_all_kitty_images() -> String {
    "\x1b_Ga=d,d=A,q=2\x1b\\".to_owned()
}

/// Unconditionally wraps verbatim text and URL in OSC 8 open/close sequences.
#[must_use]
pub fn hyperlink(text: &str, url: &str) -> String {
    format!("\x1b]8;;{url}\x1b\\{text}\x1b]8;;\x1b\\")
}
