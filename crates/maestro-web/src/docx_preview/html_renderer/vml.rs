//! VML shapes rendered as SVG.
use super::{Children, HtmlRenderer};
use crate::docx_preview::html_element::{HtmlElement, Namespace};
use maestro_app::browser::docx_preview::{
    OpenXmlElementData as Data, OpenXmlElementTree, VmlAttributeValue, VmlElement,
};

impl<'a> HtmlRenderer<'a> {
    /// Render a shape inside an SVG image carrying the shape's raw style text (`undefined`
    /// when absent); an image shape's `href` is set when its image loads.
    pub(super) fn render_vml_element(
        &mut self,
        tree: &'a OpenXmlElementTree,
        index: usize,
        shape: &VmlElement,
    ) -> HtmlElement {
        let mut container = HtmlElement::new_ns(Namespace::Svg, "svg");
        let style = shape.css_style_text.as_deref().unwrap_or("undefined");
        container.set_attribute("style", style);
        let mut result = self.render_vml_child_element(tree, index, shape);
        if let Some(url) = shape
            .image_href
            .as_ref()
            .and_then(|href| href.id.as_deref())
            .filter(|id| !id.is_empty())
            .and_then(|id| self.load_document_image(id))
        {
            result.set_attribute("href", url);
        }
        container.push_element(result);
        container
    }

    /// Render a shape as its SVG element with its attributes in order, nested shapes as SVG
    /// and other content through the ordinary dispatch.
    fn render_vml_child_element(
        &mut self,
        tree: &'a OpenXmlElementTree,
        index: usize,
        shape: &VmlElement,
    ) -> HtmlElement {
        let mut result = HtmlElement::new_ns(Namespace::Svg, shape.tag_name.as_str());
        for (name, value) in &shape.attrs {
            let value = match value {
                VmlAttributeValue::Text(text) => text.as_str(),
                VmlAttributeValue::Null => "null",
                VmlAttributeValue::Undefined => "undefined",
            };
            result.set_attribute(name, value);
        }
        for &child in &tree.nodes[index].base.children {
            match &tree.nodes[child].data {
                Data::VmlElement(nested) => {
                    let nested = self.render_vml_child_element(tree, child, nested);
                    result.push_element(nested);
                }
                _ => self.render_element(tree, child, Children::Own, &mut result.children),
            }
        }
        result
    }
}
