//! Native borrowed XML views for Word parts.
use super::super::document::common::{float_prefix, integer_prefix};
use super::super::{DocxError, LengthUsage, convert_boolean, convert_length};
use std::{ops::Range, sync::Arc};

/// Parsed XML borrowing its supplied source text.
pub struct XmlParser<'input> {
    /// Native tree, private to this adapter.
    document: roxmltree::Document<'input>,
}
/// Borrowed XML element.
#[derive(Clone, Copy)]
pub struct XmlElement<'doc, 'input: 'doc> {
    /// Native node belonging to the parser.
    node: roxmltree::Node<'doc, 'input>,
}
/// Ordinary XML attribute, excluding namespace declarations.
pub struct XmlAttribute<'doc> {
    /// Name without a namespace prefix.
    pub local_name: &'doc str,
    /// Resolved namespace URI.
    pub namespace: Option<&'doc str>,
    /// Decoded attribute value.
    pub value: &'doc str,
}

/// Parse XML, optionally removing a leading declaration; DOCTYPE is rejected.
///
/// # Errors
/// Returns an XML error for malformed remaining XML or a DOCTYPE declaration.
pub fn parse_xml_string(
    input: &str,
    trim_xml_declaration: bool,
) -> Result<XmlParser<'_>, DocxError> {
    let mut input = input.strip_prefix('\u{feff}').unwrap_or(input);
    if trim_xml_declaration {
        let trimmed = input.trim_start_matches([' ', '\t', '\r', '\n']);
        if trimmed
            .strip_prefix("<?xml")
            .is_some_and(|s| s.starts_with([' ', '\t', '\r', '\n']))
            && let Some(end) = trimmed.find("?>")
        {
            input = &trimmed[end + 2..];
        }
    }
    roxmltree::Document::parse(input)
        .map(|document| XmlParser { document })
        .map_err(|error| DocxError::Xml {
            path: None,
            message: error.to_string(),
        })
}

impl<'doc, 'input: 'doc> XmlElement<'doc, 'input> {
    /// Return the element's local name.
    #[must_use]
    pub fn local_name(&self) -> &'doc str {
        self.node.tag_name().name()
    }
    /// Return the resolved nonempty namespace URI.
    #[must_use]
    pub fn namespace(&self) -> Option<&'doc str> {
        self.node.tag_name().namespace().filter(|s| !s.is_empty())
    }
    /// Concatenate descendant text in document order.
    #[must_use]
    pub fn text_content(&self) -> String {
        self.node
            .descendants()
            .filter(roxmltree::Node::is_text)
            .filter_map(|n| n.text())
            .collect()
    }
}

impl<'input> XmlParser<'input> {
    /// Return the document element.
    #[must_use]
    pub fn root(&self) -> XmlElement<'_, 'input> {
        XmlElement {
            node: self.document.root_element(),
        }
    }
    /// Iterate immediate element children, optionally selecting a local name.
    pub fn elements<'doc>(
        &'doc self,
        element: XmlElement<'doc, 'input>,
        local_name: Option<&'doc str>,
    ) -> impl Iterator<Item = XmlElement<'doc, 'input>> + 'doc {
        element
            .node
            .children()
            .filter(move |n| {
                n.is_element() && local_name.is_none_or(|name| n.tag_name().name() == name)
            })
            .map(|node| XmlElement { node })
    }
    /// Find the first immediate element with this local name.
    #[must_use]
    pub fn element<'doc>(
        &'doc self,
        element: XmlElement<'doc, 'input>,
        local_name: &str,
    ) -> Option<XmlElement<'doc, 'input>> {
        element
            .node
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == local_name)
            .map(|node| XmlElement { node })
    }
    /// Read an attribute of the first matching child.
    #[must_use]
    pub fn element_attr<'doc>(
        &'doc self,
        element: XmlElement<'doc, 'input>,
        local_name: &str,
        attr_local_name: &str,
    ) -> Option<&'doc str> {
        self.attr(self.element(element, local_name)?, attr_local_name)
    }
    /// Iterate ordinary attributes in source order.
    pub fn attrs<'doc>(
        &'doc self,
        element: XmlElement<'doc, 'input>,
    ) -> impl Iterator<Item = XmlAttribute<'doc>> + 'doc {
        element.node.attributes().map(|a| XmlAttribute {
            local_name: a.name(),
            namespace: a.namespace().filter(|s| !s.is_empty()),
            value: a.value(),
        })
    }
    /// Read the first attribute with this local name, regardless of namespace.
    #[must_use]
    pub fn attr<'doc>(
        &'doc self,
        element: XmlElement<'doc, 'input>,
        local_name: &str,
    ) -> Option<&'doc str> {
        self.attrs(element)
            .find(|a| a.local_name == local_name)
            .map(|a| a.value)
    }
    /// Decode an attribute once while preserving authored literal XML whitespace.
    ///
    /// Unlike ordinary XML attributes, spreadsheet relationship targets retain tabs
    /// and newlines for URL parsing at their consuming boundary.
    pub(crate) fn literal_attr(
        &self,
        element: XmlElement<'_, 'input>,
        name: &str,
    ) -> Result<Option<String>, DocxError> {
        let Some(attribute) = element.node.attributes().find(|a| a.name() == name) else {
            return Ok(None);
        };
        let range = attribute.range_value();
        let source = self.document.input_text();
        let quote = &source[range.start - 1..range.start];
        let literal = source[range]
            .replace('\r', "&#13;")
            .replace('\n', "&#10;")
            .replace('\t', "&#9;");
        let text = format!("<v a={quote}{literal}{quote}/>");
        let xml = parse_xml_string(&text, false)?;
        Ok(xml.attr(xml.root(), "a").map(str::to_owned))
    }
    /// Read a signed integer prefix, using the default for absent or empty text.
    #[must_use]
    pub fn int_attr(
        &self,
        element: XmlElement<'_, 'input>,
        name: &str,
        default_value: Option<f64>,
    ) -> Option<f64> {
        self.number_attr(element, name, default_value, |s| integer_prefix(s, 10))
    }
    /// Read a hexadecimal prefix, using the default for absent or empty text.
    #[must_use]
    pub fn hex_attr(
        &self,
        element: XmlElement<'_, 'input>,
        name: &str,
        default_value: Option<f64>,
    ) -> Option<f64> {
        self.number_attr(element, name, default_value, |s| integer_prefix(s, 16))
    }
    /// Read a floating-point prefix, using the default for absent or empty text.
    #[must_use]
    pub fn float_attr(
        &self,
        element: XmlElement<'_, 'input>,
        name: &str,
        default_value: Option<f64>,
    ) -> Option<f64> {
        self.number_attr(element, name, default_value, float_prefix)
    }
    /// Read a numeric attribute through the selected prefix conversion.
    fn number_attr(
        &self,
        element: XmlElement<'_, 'input>,
        name: &str,
        default_value: Option<f64>,
        parse: fn(&str) -> f64,
    ) -> Option<f64> {
        self.attr(element, name)
            .filter(|s| !s.is_empty())
            .map(parse)
            .or(default_value)
    }
    /// Read a boolean token with an explicit fallback.
    #[must_use]
    pub fn bool_attr(
        &self,
        element: XmlElement<'_, 'input>,
        name: &str,
        default_value: Option<bool>,
    ) -> Option<bool> {
        convert_boolean(self.attr(element, name), default_value)
    }
    /// Read a length using the selected unit policy.
    #[must_use]
    pub fn length_attr(
        &self,
        element: XmlElement<'_, 'input>,
        name: &str,
        usage: LengthUsage,
    ) -> Option<String> {
        convert_length(self.attr(element, name), usage)
    }
}

/// Decode one text fragment, preserving literal carriage returns and raw failures.
pub(crate) fn decode_text(text: &str) -> String {
    let fragment = format!("<t>{}</t>", text.replace('\r', "&#13;"));
    parse_xml_string(&fragment, false)
        .map_or_else(|_| text.to_owned(), |xml| xml.root().text_content())
}

/// Selected element retaining its complete admitted XML source.
#[derive(Debug, Clone)]
pub struct OwnedXmlElement {
    /// Shared complete part source, including ancestor namespaces.
    source: Arc<str>,
    /// Element byte range within that source.
    range: Range<usize>,
}
impl OwnedXmlElement {
    /// Capture a borrowed element and its admitted source together.
    #[must_use]
    pub fn new(element: XmlElement<'_, '_>) -> Self {
        Self {
            source: Arc::from(element.node.document().input_text()),
            range: element.source_range(),
        }
    }
    /// Return the complete admitted source, not a standalone fragment.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }
    /// Return the selected element's byte range within [`Self::source`].
    #[must_use]
    pub fn source_range(&self) -> Range<usize> {
        self.range.clone()
    }
    /// Retain another element from the same parsed source allocation.
    pub(crate) fn slice(&self, element: XmlElement<'_, '_>) -> Self {
        Self {
            source: Arc::clone(&self.source),
            range: element.source_range(),
        }
    }
}
impl XmlElement<'_, '_> {
    /// Return the element's byte range in the parser's admitted source.
    #[must_use]
    pub fn source_range(&self) -> Range<usize> {
        self.node.range()
    }
}
impl<'input> XmlParser<'input> {
    /// Locate an owned selection in its reparsed complete source.
    pub(crate) fn selected(
        &self,
        input: &OwnedXmlElement,
    ) -> Result<XmlElement<'_, 'input>, DocxError> {
        self.document
            .descendants()
            .find(|node| node.is_element() && node.range() == input.range)
            .map(|node| XmlElement { node })
            .ok_or_else(|| DocxError::Xml {
                path: None,
                message: "Selected XML element is missing".into(),
            })
    }
}
