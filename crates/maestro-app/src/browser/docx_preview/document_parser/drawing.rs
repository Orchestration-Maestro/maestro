//! Drawing wrapper selection and style projection.
use super::super::{LengthUsage, convert_length};
use super::{Data, DocumentParser, DocxError, XmlElement, XmlParser};
use indexmap::IndexMap;

/// Final supported text wrapping policy.
#[derive(Default, Clone, Copy)]
enum Wrap {
    /// Ordinary inline or floating drawing.
    #[default]
    Ordinary,
    /// Full-width block.
    TopAndBottom,
    /// Collapsed, relatively positioned block.
    None,
}
/// Horizontal alignment and independently selected offsets.
struct Position {
    /// Final horizontal alignment text.
    align: String,
    /// Horizontal offset, absent if conversion was unavailable.
    x: Option<String>,
    /// Vertical offset, absent if conversion was unavailable.
    y: Option<String>,
}
impl Position {
    /// Select either simple coordinates or axis-specific positioning.
    fn parse(xml: &super::XmlParser<'_>, wrapper: XmlElement<'_, '_>) -> Self {
        let mut result = Self {
            align: "left".into(),
            x: Some("0".into()),
            y: Some("0".into()),
        };
        let simple = xml.bool_attr(wrapper, "simplePos", None) == Some(true);
        for node in xml.elements(wrapper, None) {
            match node.local_name() {
                "simplePos" if simple => {
                    result.x = xml.length_attr(node, "x", LengthUsage::Emu);
                    result.y = xml.length_attr(node, "y", LengthUsage::Emu);
                }
                "positionH" | "positionV" if !simple => {
                    result.axis(xml, node);
                }
                _ => {}
            }
        }
        result
    }
    /// Update one axis while retaining an earlier alignment or offset when absent.
    fn axis(&mut self, xml: &XmlParser<'_>, node: XmlElement<'_, '_>) {
        let horizontal = node.local_name() == "positionH";
        if horizontal && let Some(align) = xml.element(node, "align") {
            self.align = align.text_content();
        }
        if let Some(offset) = xml.element(node, "posOffset") {
            let value = convert_length(Some(&offset.text_content()), LengthUsage::Emu);
            if horizontal {
                self.x = value;
            } else {
                self.y = value;
            }
        }
    }
    /// Apply final wrapping without changing earlier style insertion positions.
    fn apply(self, wrap: Wrap, anchor: bool, style: &mut IndexMap<String, Option<String>>) {
        match wrap {
            Wrap::TopAndBottom => {
                style.insert("display".into(), Some("block".into()));
                if !self.align.is_empty() {
                    style.insert("text-align".into(), Some(self.align));
                    style.insert("width".into(), Some("100%".into()));
                }
            }
            Wrap::None => {
                for (key, value) in [
                    ("display", "block"),
                    ("position", "relative"),
                    ("width", "0px"),
                    ("height", "0px"),
                ] {
                    style.insert(key.into(), Some(value.into()));
                }
                style.extend([("left", self.x), ("top", self.y)].into_iter().filter_map(
                    |(key, value)| {
                        value
                            .filter(|v| !v.is_empty())
                            .map(|value| (key.into(), Some(value)))
                    },
                ));
            }
            Wrap::Ordinary if anchor && matches!(self.align.as_str(), "left" | "right") => {
                style.insert("float".into(), Some(self.align));
            }
            Wrap::Ordinary => {}
        }
    }
}
impl<'input> DocumentParser<'_, 'input> {
    /// Select the first immediate supported wrapper.
    pub(super) fn drawing(
        &mut self,
        element: XmlElement<'_, 'input>,
    ) -> Result<Option<usize>, DocxError> {
        let Some(wrapper) = self
            .xml
            .elements(element, None)
            .find(|child| matches!(child.local_name(), "inline" | "anchor"))
        else {
            return Ok(None);
        };
        let mut css_style = IndexMap::new();
        let mut wrap = Wrap::default();
        let mut children = Vec::new();
        for child in self.xml.elements(wrapper, None) {
            match child.local_name() {
                "extent" => {
                    geometry(
                        self.xml,
                        child,
                        &mut css_style,
                        [("width", "cx"), ("height", "cy")],
                    );
                }
                "graphic" => children.extend(self.graphic(child)?),
                "wrapTopAndBottom" => wrap = Wrap::TopAndBottom,
                "wrapNone" => wrap = Wrap::None,
                _ => {}
            }
        }
        Position::parse(self.xml, wrapper).apply(
            wrap,
            wrapper.local_name() == "anchor",
            &mut css_style,
        );
        let id = self.push(Data::Drawing { css_style });
        self.tree.nodes[id].base.children = children;
        Ok(Some(id))
    }
}

/// Select required picture structure without publishing a partial tree.
fn required<'doc, 'input: 'doc>(
    xml: &'doc XmlParser<'input>,
    element: XmlElement<'doc, 'input>,
    name: &str,
) -> Result<XmlElement<'doc, 'input>, DocxError> {
    xml.element(element, name).ok_or_else(|| DocxError::Xml {
        path: None,
        message: format!("Missing picture element: {name}"),
    })
}
impl<'input> DocumentParser<'_, 'input> {
    /// Parse the first immediate picture in required graphic data.
    fn graphic(&mut self, element: XmlElement<'_, 'input>) -> Result<Option<usize>, DocxError> {
        let data = required(self.xml, element, "graphicData")?;
        self.xml
            .element(data, "pic")
            .map(|picture| self.picture(picture))
            .transpose()
    }
    /// Read the first relationship-bearing picture structure.
    fn picture(&mut self, element: XmlElement<'_, 'input>) -> Result<usize, DocxError> {
        let fill = required(self.xml, element, "blipFill")?;
        let blip = required(self.xml, fill, "blip")?;
        let shape = required(self.xml, element, "spPr")?;
        let rotation = self
            .xml
            .element(shape, "xfrm")
            .map(|node| self.xml.int_attr(node, "rot", Some(0.0)).unwrap_or(0.0) / 60_000.0);
        let src_rect = self.xml.element(fill, "srcRect").map(|node| {
            ["l", "t", "r", "b"]
                .map(|axis| self.xml.int_attr(node, axis, Some(0.0)).unwrap_or(0.0) / 100_000.0)
        });
        let mut css_style = [("position".into(), Some("relative".into()))].into();
        if let Some(transform) = self.xml.element(shape, "xfrm") {
            for child in self.xml.elements(transform, None) {
                let axes = match child.local_name() {
                    "ext" => [("width", "cx"), ("height", "cy")],
                    "off" => [("left", "x"), ("top", "y")],
                    _ => continue,
                };
                geometry(self.xml, child, &mut css_style, axes);
            }
        }
        Ok(self.push(Data::Image {
            src: self.attr(blip, "embed"),
            css_style,
            src_rect,
            rotation,
        }))
    }
}

/// Replace a pair of converted axes, preserving existing key positions.
fn geometry(
    xml: &XmlParser<'_>,
    element: XmlElement<'_, '_>,
    style: &mut IndexMap<String, Option<String>>,
    axes: [(&str, &str); 2],
) {
    for (key, attribute) in axes {
        style.insert(
            key.into(),
            xml.length_attr(element, attribute, LengthUsage::Emu),
        );
    }
}
