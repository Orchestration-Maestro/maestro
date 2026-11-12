//! Owned VML shapes and recursive body delegation.
use super::{
    DocxError, OpenXmlElementData as Data, XmlElement, XmlParser, document_parser::DocumentParser,
};
use indexmap::IndexMap;

/// Supported SVG tag projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmlTag {
    /// Rectangle.
    Rect,
    /// Ellipse.
    Ellipse,
    /// Line.
    Line,
    /// Generic shape group.
    G,
    /// Embedded document content.
    ForeignObject,
    /// Relationship-backed image.
    Image,
}
impl VmlTag {
    /// Return the SVG tag spelling.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rect => "rect",
            Self::Ellipse => "ellipse",
            Self::Line => "line",
            Self::G => "g",
            Self::ForeignObject => "foreignObject",
            Self::Image => "image",
        }
    }
}
/// Literal, nullable or unavailable SVG attribute.
#[derive(Debug)]
pub enum VmlAttributeValue {
    /// Exact decoded attribute text.
    Text(String),
    /// Missing selected attribute.
    Null,
    /// Unavailable second coordinate.
    Undefined,
}
/// Owned image relationship metadata.
#[derive(Debug)]
pub struct VmlImageHref {
    /// Nullable relationship identifier.
    pub id: Option<String>,
    /// Nullable image title.
    pub title: Option<String>,
}
/// Shape payload independent of its XML input.
#[derive(Debug)]
pub struct VmlElement {
    /// Selected SVG tag.
    pub tag_name: VmlTag,
    /// Ordered SVG attributes.
    pub attrs: IndexMap<String, VmlAttributeValue>,
    /// Inherited ordered style map.
    pub css_style: IndexMap<String, Option<String>>,
    /// Optional unparsed inline style text.
    pub css_style_text: Option<String>,
    /// Optional image relationship metadata.
    pub image_href: Option<VmlImageHref>,
}
impl VmlElement {
    /// Construct only recognized shapes with their tag-specific defaults.
    fn new(name: &str) -> Option<Self> {
        let tag_name = match name {
            "rect" => VmlTag::Rect,
            "oval" => VmlTag::Ellipse,
            "line" => VmlTag::Line,
            "shape" => VmlTag::G,
            "textbox" => VmlTag::ForeignObject,
            _ => return None,
        };
        let mut result = Self {
            tag_name,
            attrs: IndexMap::new(),
            css_style: IndexMap::new(),
            css_style_text: None,
            image_href: None,
        };
        match tag_name {
            VmlTag::Rect | VmlTag::ForeignObject => result.image_size(),
            VmlTag::Ellipse => {
                for key in ["cx", "cy", "rx", "ry"] {
                    result
                        .attrs
                        .insert(key.into(), VmlAttributeValue::Text("50%".into()));
                }
            }
            _ => {}
        }
        Some(result)
    }
    /// Read enumerated attributes in source order rather than first-match query order.
    fn attributes(&mut self, xml: &XmlParser<'_>, element: XmlElement<'_, '_>) {
        for attribute in xml.attrs(element) {
            match attribute.local_name {
                "style" => self.css_style_text = Some(attribute.value.into()),
                "fillcolor" => {
                    self.attrs.insert(
                        "fill".into(),
                        VmlAttributeValue::Text(attribute.value.into()),
                    );
                }
                "from" => self.point(attribute.value, ["x1", "y1"]),
                "to" => self.point(attribute.value, ["x2", "y2"]),
                _ => {}
            }
        }
    }
    /// Retain the first two literal comma-separated coordinates.
    fn point(&mut self, value: &str, keys: [&str; 2]) {
        let mut components = value.split(',');
        for key in keys {
            self.attrs.insert(
                key.into(),
                components.next().map_or(VmlAttributeValue::Undefined, |v| {
                    VmlAttributeValue::Text(v.into())
                }),
            );
        }
    }
    /// Replace stroke attributes with first-local-name values and an absent-width default.
    fn stroke(&mut self, xml: &XmlParser<'_>, element: XmlElement<'_, '_>) {
        self.attrs.insert(
            "stroke".into(),
            xml.attr(element, "color")
                .map_or(VmlAttributeValue::Null, |v| {
                    VmlAttributeValue::Text(v.into())
                }),
        );
        self.attrs.insert(
            "stroke-width".into(),
            VmlAttributeValue::Text(
                xml.length_attr(element, "weight", super::LengthUsage::Emu)
                    .unwrap_or_else(|| "1px".into()),
            ),
        );
    }
    /// Assign full-size dimensions while retaining earlier attribute positions.
    fn image_size(&mut self) {
        for key in ["width", "height"] {
            self.attrs
                .insert(key.into(), VmlAttributeValue::Text("100%".into()));
        }
    }
}
impl<'input> DocumentParser<'_, 'input> {
    /// Parse the direct recognized shapes of a VML picture.
    pub(super) fn vml_picture(
        &mut self,
        element: XmlElement<'_, 'input>,
    ) -> Result<usize, DocxError> {
        let id = self.push(Data::VmlPicture);
        let mut children = Vec::new();
        for child in self.xml.elements(element, None) {
            if let Some(child) = self.vml_element(child)? {
                children.push(child);
            }
        }
        self.tree.nodes[id].base.children = children;
        Ok(id)
    }
    /// Parse one recognized shape without flattening unknown wrappers.
    fn vml_element(&mut self, element: XmlElement<'_, 'input>) -> Result<Option<usize>, DocxError> {
        let Some(mut result) = VmlElement::new(element.local_name()) else {
            return Ok(None);
        };
        result.attributes(self.xml, element);
        let mut children = Vec::new();
        for child in self.xml.elements(element, None) {
            match child.local_name() {
                "stroke" => result.stroke(self.xml, child),
                "imagedata" => {
                    result.tag_name = VmlTag::Image;
                    result.image_size();
                    result.image_href = Some(VmlImageHref {
                        id: self.attr(child, "id"),
                        title: self.attr(child, "title"),
                    });
                }
                "txbxContent" => children.extend(self.body(child)?),
                "fill" => {}
                _ => children.extend(self.vml_element(child)?),
            }
        }
        let id = self.push(Data::VmlElement(result));
        self.tree.nodes[id].base.children = children;
        Ok(Some(id))
    }
}
