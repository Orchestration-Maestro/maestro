//! Drawing boxes, their images, and alternate format chunks.
use super::{CssStyle, HtmlRenderer, render_style_values};
use crate::docx_preview::html_element::HtmlElement;
use maestro_app::browser::docx_preview::OpenXmlElementTree;

impl<'a> HtmlRenderer<'a> {
    /// Render a drawing as an inline box holding its content, then its own declarations.
    pub(super) fn render_drawing(
        &mut self,
        tree: &'a OpenXmlElementTree,
        index: usize,
        css_style: &CssStyle,
    ) -> HtmlElement {
        let mut result = self.render_container(tree, index, "div");
        result.set_style("display", Some("inline-block"));
        result.set_style("position", Some("relative"));
        result.set_style("text-indent", Some("0px"));
        render_style_values(Some(css_style), &mut result);
        result
    }

    /// Render a picture with its declarations: a crop with any nonzero edge scales and clips
    /// it, a rotation turns it, and its source is the loaded image.
    pub(super) fn render_image(
        &mut self,
        src: Option<&str>,
        css_style: &CssStyle,
        src_rect: Option<[f64; 4]>,
        rotation: Option<f64>,
    ) -> HtmlElement {
        let mut result = HtmlElement::new("img");
        render_style_values(Some(css_style), &mut result);
        let mut transform = None;
        if let Some([left, top, right, bottom]) =
            src_rect.filter(|edges| edges.iter().any(|edge| *edge != 0.0))
        {
            transform = Some(format!(
                "scale({}, {})",
                number(1.0 / (1.0 - left - right)),
                number(1.0 / (1.0 - top - bottom))
            ));
            let clip = format!(
                "rect({}% {}% {}% {}%)",
                fixed(100.0 * top),
                fixed(100.0 * (1.0 - right)),
                fixed(100.0 * (1.0 - bottom)),
                fixed(100.0 * left)
            );
            result.set_style("clip-path", Some(clip));
        }
        if let Some(rotation) = rotation.filter(|rotation| *rotation != 0.0 && !rotation.is_nan()) {
            let scale = transform.unwrap_or_default();
            transform = Some(format!("rotate({}deg) {scale}", number(rotation)));
        }
        if let Some(transform) = transform {
            result.set_style("transform", Some(transform.trim()));
        }
        if let Some(url) = src.and_then(|id| self.load_document_image(id)) {
            result.set_attribute("src", url);
        }
        result
    }

    /// Render an alternate format chunk in a frame that shows its text without running
    /// scripts, unless chunks are not rendered.
    pub(super) fn render_alt_chunk(&mut self, id: Option<&str>) -> Option<HtmlElement> {
        if !self.options().resources.render_alt_chunks {
            return None;
        }
        let mut result = HtmlElement::new("iframe");
        result.set_attribute("sandbox", "");
        if let Some(text) = id.and_then(|id| self.load_alt_chunk(id)) {
            result.set_attribute("srcdoc", text);
        }
        Some(result)
    }
}

/// Write a number as script number text does.
fn number(value: f64) -> String {
    ryu_js::Buffer::new().format(value).to_owned()
}

/// Write a number with two fixed decimals as script fixed-point text does.
fn fixed(value: f64) -> String {
    ryu_js::Buffer::new().format_to_fixed(value, 2).to_owned()
}
