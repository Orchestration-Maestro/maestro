//! SVG primitives shared by DOM and native glyph rendering.
use indexmap::IndexMap;
use maestro_app::markup::{EscapeMode, escape_into};
use num_traits::ToPrimitive;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Literal SVG attribute value.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SvgValue {
    /// Literal text.
    Text(String),
    /// Numeric attribute.
    Number(f64),
}
/// Ordered primitive names with their ordered attributes.
pub type IconNode = Vec<(String, IndexMap<String, SvgValue>)>;
/// Decode the used glyph catalog.
/// # Errors
/// Returns an embedded geometry decoding failure.
pub fn icons() -> Result<IndexMap<String, IconNode>, serde_json::Error> {
    serde_json::from_str(include_str!("../assets/icons.json"))
}

/// Relative glyph size at the caller's normal root font size.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum IconSize {
    /// Twelve pixels.
    Xs,
    /// Sixteen pixels.
    Sm,
    /// Twenty pixels.
    #[default]
    Md,
    /// Twenty-four pixels.
    Lg,
    /// Thirty-two pixels.
    Xl,
}
impl IconSize {
    /// Geometry scale and utility-class suffix.
    fn dimensions(self) -> (u8, &'static str) {
        match self {
            Self::Xs => (12, "3"),
            Self::Sm => (16, "4"),
            Self::Md => (20, "5"),
            Self::Lg => (24, "6"),
            Self::Xl => (32, "8"),
        }
    }
}
/// Rem-based widths and heights for the emitted utility classes.
pub const ICON_STYLES: &str = include_str!("../assets/icons.css");
/// Append a literal XML attribute using the application markup owner.
fn attribute(svg: &mut String, key: &str, value: &str) {
    svg.push(' ');
    escape_into(key, svg, EscapeMode::Xml);
    svg.push_str("=\"");
    escape_into(value, svg, EscapeMode::Xml);
    svg.push('"');
}
/// Build the SVG shared by DOM parsing and native rasterization.
#[must_use]
pub fn icon_svg(node: &IconNode, size: IconSize, class_name: Option<&str>) -> String {
    let mut svg = String::from(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"24\" height=\"24\" viewBox=\"0 0 24 24\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\"",
    );
    let (_, suffix) = size.dimensions();
    let mut classes = format!("w-{suffix} h-{suffix}");
    if let Some(class) = class_name.filter(|s| !s.is_empty()) {
        classes.push(' ');
        classes.push_str(class);
    }
    attribute(&mut svg, "class", &classes);
    svg.push('>');
    for (tag, attrs) in node {
        svg.push('<');
        escape_into(tag, &mut svg, EscapeMode::Xml);
        for (key, value) in attrs {
            match value {
                SvgValue::Text(text) => attribute(&mut svg, key, text),
                SvgValue::Number(number) => {
                    attribute(&mut svg, key, ryu_js::Buffer::new().format(*number));
                }
            }
        }
        svg.push_str("/>");
    }
    svg.push_str("</svg>");
    svg
}

/// Native icon rendering failure.
#[derive(Debug)]
pub struct IconError {
    /// Original parser failure, if any.
    cause: Option<usvg::Error>,
}
impl fmt::Display for IconError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.cause {
            Some(error) => error.fmt(f),
            None => f.write_str("Unrepresentable icon canvas"),
        }
    }
}
impl std::error::Error for IconError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.cause
            .as_ref()
            .map(|error| error as &dyn std::error::Error)
    }
}
/// Rasterize the shared SVG at the requested density using caller-supplied color.
/// # Errors
/// Returns the original SVG parsing failure or an unrepresentable canvas error.
pub fn icon(
    node: &IconNode,
    size: IconSize,
    color: egui::Color32,
    pixels_per_point: f32,
) -> Result<egui::ColorImage, IconError> {
    let points = f32::from(size.dimensions().0);
    let pixels = (points * pixels_per_point)
        .ceil()
        .to_u32()
        .filter(|n| *n > 0)
        .ok_or(IconError { cause: None })?;
    let [r, g, b, a] = color.to_srgba_unmultiplied();
    let options = usvg::Options {
        style_sheet: Some(format!(
            "svg {{ color: rgb({r},{g},{b}); opacity: {}; }}",
            f32::from(a) / 255.0
        )),
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_str(&icon_svg(node, size, None), &options)
        .map_err(|cause| IconError { cause: Some(cause) })?;
    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(pixels, pixels).ok_or(IconError { cause: None })?;
    let scale = (points * pixels_per_point).ceil() / 24.0;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let width = usize::try_from(pixels).map_err(|_| IconError { cause: None })?;
    Ok(egui::ColorImage::from_rgba_premultiplied(
        [width, width],
        pixmap.data(),
    ))
}

/// Parse the shared SVG into a real namespaced browser element.
/// # Errors
/// Propagates DOM parsing and element conversion failures.
#[cfg(target_arch = "wasm32")]
pub fn icon_dom(
    node: &IconNode,
    size: IconSize,
    class_name: Option<&str>,
) -> Result<web_sys::SvgElement, wasm_bindgen::JsValue> {
    use wasm_bindgen::JsCast;
    let document = web_sys::DomParser::new()?.parse_from_string(
        &icon_svg(node, size, class_name),
        web_sys::SupportedType::ImageSvgXml,
    )?;
    document
        .document_element()
        .ok_or_else(|| wasm_bindgen::JsValue::from_str("SVG has no root"))?
        .dyn_into()
        .map_err(Into::into)
}
