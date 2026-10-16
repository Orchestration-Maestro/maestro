//! Authored custom property values.
use super::super::{DocxError, XmlElement, XmlParser};
/// One custom value without inferred scalar conversions.
#[derive(Debug)]
pub struct CustomProperty {
    /// Nullable format identifier.
    pub format_id: Option<String>,
    /// Nullable authored property name.
    pub name: Option<String>,
    /// Authored qualified value element name.
    pub r#type: String,
    /// Descendant value text.
    pub value: String,
}
/// Select the first value element, excluding nonvalue XML nodes.
pub(super) fn parse_custom_props<'doc, 'input: 'doc>(
    xml: &'doc XmlParser<'input>,
    root: XmlElement<'doc, 'input>,
) -> Result<Vec<CustomProperty>, DocxError> {
    xml.elements(root, Some("property"))
        .map(|property| {
            let value = xml
                .elements(property, None)
                .next()
                .ok_or_else(|| DocxError::Xml {
                    path: None,
                    message: "Custom property has no value element".into(),
                })?;
            Ok(CustomProperty {
                format_id: xml.attr(property, "fmtid").map(str::to_owned),
                name: xml.attr(property, "name").map(str::to_owned),
                r#type: value.qualified_name().into(),
                value: value.text_content(),
            })
        })
        .collect()
}
