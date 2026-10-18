//! Native inline document emission from the existing owned arena.
use super::{HtmlRenderer, numbering::presence};
use maestro_app::browser::docx_preview::{
    CommonProperties, OpenXmlElementData as Data, OpenXmlElementTree, VerticalAlign,
};
use wasm_bindgen::JsValue;
use web_sys::{Document, Element, Node};

impl HtmlRenderer {
    /// Render selected arena positions in encounter order.
    pub(crate) fn render_elements(
        &self,
        document: &Document,
        tree: &mut OpenXmlElementTree,
        indices: &[usize],
    ) -> Result<Vec<Node>, JsValue> {
        let mut nodes = Vec::new();
        for &index in indices {
            nodes.extend(self.render_element(document, tree, index)?);
        }
        Ok(nodes)
    }

    /// Dispatch independent inline elements, leaving other document owners unselected.
    pub(crate) fn render_element(
        &self,
        document: &Document,
        tree: &mut OpenXmlElementTree,
        index: usize,
    ) -> Result<Vec<Node>, JsValue> {
        if matches!(tree.nodes[index].data, Data::Inserted) {
            let children = tree.nodes[index].base.children.clone();
            return self.render_elements(document, tree, &children);
        }
        let node: Option<Node> = match &tree.nodes[index].data {
            Data::Paragraph { .. } => Some(self.render_paragraph(document, tree, index)?.into()),
            Data::Run { .. } => self.render_run(document, tree, index)?.map(Into::into),
            Data::Text { text } => Some(document.create_text_node(text).into()),
            Data::Break { r#break } if r#break == "textWrapping" => {
                Some(Self::create_element(document, "br", &[])?.into())
            }
            Data::NoBreakHyphen => Some(document.create_text_node("\u{2011}").into()),
            Data::Tab => Some(
                Self::create_element(
                    document,
                    "span",
                    &[document.create_text_node("\u{2003}").into()],
                )?
                .into(),
            ),
            Data::BookmarkStart { name, .. } => {
                let element = Self::create_element(document, "span", &[])?;
                element.set_id(name.as_deref().unwrap_or("null"));
                Some(element.into())
            }
            Data::Symbol { font, char } => {
                let element = Self::create_element(document, "span", &[])?;
                js_sys::Reflect::set(
                    native_style(&element)?.as_ref(),
                    &JsValue::from_str("fontFamily"),
                    &font.as_deref().map_or(JsValue::NULL, JsValue::from_str),
                )?;
                element.set_inner_html(&format!("&#x{};", char.as_deref().unwrap_or("null")));
                Some(element.into())
            }
            _ => None,
        };
        Ok(node.into_iter().collect())
    }

    /// Render paragraph children before applying paragraph presentation.
    pub(crate) fn render_paragraph(
        &self,
        document: &Document,
        tree: &mut OpenXmlElementTree,
        index: usize,
    ) -> Result<Element, JsValue> {
        let children = tree.nodes[index].base.children.clone();
        let nodes = self.render_elements(document, tree, &children)?;
        let output = Self::create_element(document, "p", &nodes)?;
        let Data::Paragraph { properties } = &mut tree.nodes[index].data else {
            return Ok(output);
        };
        let style_name = properties.style_name.as_ref().and_then(|v| v.as_deref());
        let style = self
            .find_style(style_name)
            .and_then(|s| s.paragraph_props.as_ref());
        if properties.tabs.is_none() {
            properties.tabs = style.and_then(|s| s.tabs.as_ref()).cloned();
        }
        self.render_class(properties.class_name.as_deref(), style_name, &output)?;
        Self::render_style_values(properties.css_style.as_ref(), &output)?;
        Self::render_common_properties(&properties.common, &output)?;
        if let Some(numbering) = properties
            .numbering
            .as_ref()
            .or_else(|| style.and_then(|s| s.numbering.as_ref()))
        {
            output.class_list().add_1(&self.numbering_class(
                presence(numbering.id.as_ref().map(|v| v.as_deref()).as_ref()),
                presence(numbering.level.as_ref()),
            ))?;
        }
        Ok(output)
    }

    /// Render a non-field run as a native span.
    pub(crate) fn render_run(
        &self,
        document: &Document,
        tree: &mut OpenXmlElementTree,
        index: usize,
    ) -> Result<Option<Element>, JsValue> {
        if matches!(
            tree.nodes[index].data,
            Data::Run {
                field_run: true,
                ..
            }
        ) {
            return Ok(None);
        }
        let output = Self::create_element(document, "span", &[])?;
        let Data::Run { properties, .. } = &tree.nodes[index].data else {
            return Ok(None);
        };
        self.render_class(
            None,
            properties.style_name.as_ref().and_then(|v| v.as_deref()),
            &output,
        )?;
        Self::render_style_values(properties.css_style.as_ref(), &output)?;
        let tag = properties
            .vertical_align
            .as_ref()
            .and_then(Option::as_ref)
            .map(|alignment| match alignment {
                VerticalAlign::Sub => "sub",
                VerticalAlign::Sup => "sup",
            });
        let children = tree.nodes[index].base.children.clone();
        let nodes = self.render_elements(document, tree, &children)?;
        if let Some(tag) = tag {
            output.append_child(Self::create_element(document, tag, &nodes)?.as_ref())?;
        } else {
            for node in &nodes {
                output.append_child(node)?;
            }
        }
        Ok(Some(output))
    }

    /// Create a native HTML element and append its already owned children.
    pub(crate) fn create_element(
        document: &Document,
        tag_name: &str,
        children: &[Node],
    ) -> Result<Element, JsValue> {
        let element = document.create_element(tag_name)?;
        for child in children {
            element.append_child(child)?;
        }
        Ok(element)
    }
}

/// Borrow the browser-owned declaration interface without serializing a style map.
fn native_style(element: &Element) -> Result<web_sys::CssStyleDeclaration, JsValue> {
    use wasm_bindgen::JsCast;
    js_sys::Reflect::get(element, &JsValue::from_str("style"))?.dyn_into()
}

impl HtmlRenderer {
    /// Apply ordered attributes and browser-native CSS assignments.
    pub(crate) fn render_style_values(
        values: Option<&indexmap::IndexMap<String, Option<String>>>,
        output: &Element,
    ) -> Result<(), JsValue> {
        let Some(values) = values else { return Ok(()) };
        let style = native_style(output)?;
        for (key, value) in values {
            if let Some(attribute) = key.strip_prefix('$') {
                output.set_attribute(attribute, value.as_deref().unwrap_or("null"))?;
            } else {
                js_sys::Reflect::set(
                    style.as_ref(),
                    &JsValue::from_str(key),
                    &value.as_deref().map_or(JsValue::NULL, JsValue::from_str),
                )?;
            }
        }
        Ok(())
    }

    /// Apply nonempty common values after converted declarations.
    pub(crate) fn render_common_properties(
        props: &CommonProperties,
        output: &Element,
    ) -> Result<(), JsValue> {
        let style = native_style(output)?;
        for (key, value) in [("color", &props.color), ("font-size", &props.font_size)] {
            if let Some(value) = value
                .as_ref()
                .and_then(|v| v.as_deref())
                .filter(|s| !s.is_empty())
            {
                style.set_property(key, value)?;
            }
        }
        Ok(())
    }
}

impl HtmlRenderer {
    /// Assign authored classes before appending the converted native style token.
    pub(crate) fn render_class(
        &self,
        class_name: Option<&str>,
        style_name: Option<&str>,
        output: &Element,
    ) -> Result<(), JsValue> {
        if let Some(class_name) = class_name.filter(|s| !s.is_empty()) {
            output.set_class_name(class_name);
        }
        if let Some(style_name) = style_name.filter(|s| !s.is_empty()) {
            output
                .class_list()
                .add_1(&self.process_style_name(Some(style_name)))?;
        }
        Ok(())
    }
}
