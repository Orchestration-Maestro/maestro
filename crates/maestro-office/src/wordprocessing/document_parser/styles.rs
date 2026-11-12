//! Immediate named and default Word style readers.
use super::super::document::{paragraph::parse_paragraph_properties, run::parse_run_properties};
use super::super::{DocxError, IDomStyle, IDomSubStyle, XmlElement, XmlParser};
use super::{DefaultPropertiesOptions, parse_default_properties, parse_table_style};

/// Read immediate named and default definitions in source order.
pub(in super::super) fn parse_styles_file<'doc, 'input: 'doc>(
    xml: &'doc XmlParser<'input>,
    element: XmlElement<'doc, 'input>,
    ignore_width: bool,
) -> Result<Vec<IDomStyle>, DocxError> {
    xml.elements(element, None)
        .filter_map(|child| match child.local_name() {
            "style" => Some(parse_style(xml, child, ignore_width)),
            "docDefaults" => Some(Ok(parse_default_styles(xml, child, ignore_width))),
            _ => None,
        })
        .collect()
}
/// Prepare one declaration through the shared CSS dispatcher.
fn substyle<'doc, 'input: 'doc>(
    xml: &'doc XmlParser<'input>,
    element: XmlElement<'doc, 'input>,
    target: &str,
    ignore_width: bool,
) -> IDomSubStyle {
    let mut result = IDomSubStyle {
        target: target.into(),
        ..Default::default()
    };
    parse_default_properties(
        xml,
        element,
        &mut result.values,
        None,
        DefaultPropertiesOptions {
            ignore_width,
            handler: None,
        },
    );
    result
}
/// Select each first nested property group without inventing absent declarations.
fn parse_default_styles<'doc, 'input: 'doc>(
    xml: &'doc XmlParser<'input>,
    element: XmlElement<'doc, 'input>,
    ignore_width: bool,
) -> IDomStyle {
    let mut result = IDomStyle::default();
    for child in xml.elements(element, None) {
        let (name, target) = match child.local_name() {
            "rPrDefault" => ("rPr", "span"),
            "pPrDefault" => ("pPr", "p"),
            _ => continue,
        };
        if let Some(properties) = xml.element(child, name) {
            result
                .styles
                .push(substyle(xml, properties, target, ignore_width));
        }
    }
    result
}
/// Read metadata and append independent CSS declarations for one named style.
fn parse_style<'doc, 'input: 'doc>(
    xml: &'doc XmlParser<'input>,
    element: XmlElement<'doc, 'input>,
    ignore_width: bool,
) -> Result<IDomStyle, DocxError> {
    let mut result = IDomStyle {
        id: xml.attr(element, "styleId").map(str::to_owned),
        is_default: Some(xml.bool_attr(element, "default", None)),
        linked: Some(None),
        target: match xml.attr(element, "type") {
            Some("paragraph") => Some("p".into()),
            Some("table") => Some("table".into()),
            Some("character") => Some("span".into()),
            _ => None,
        },
        ..Default::default()
    };
    for child in xml.elements(element, None) {
        match child.local_name() {
            "basedOn" => result.based_on = xml.attr(child, "val").map(str::to_owned),
            "name" => result.name = xml.attr(child, "val").map(str::to_owned),
            "link" => result.linked = Some(xml.attr(child, "val").map(str::to_owned)),
            "next" => result.next = Some(xml.attr(child, "val").map(str::to_owned)),
            "aliases" => {
                let aliases = xml.attr(child, "val").ok_or_else(|| DocxError::Xml {
                    path: None,
                    message: "Style aliases require a value".into(),
                })?;
                result.aliases = Some(aliases.split(',').map(str::to_owned).collect());
            }
            "pPr" => {
                result.styles.push(substyle(xml, child, "p", ignore_width));
                result.paragraph_props = Some(parse_paragraph_properties(xml, child));
            }
            "rPr" => {
                result
                    .styles
                    .push(substyle(xml, child, "span", ignore_width));
                result.run_props = Some(parse_run_properties(xml, child));
            }
            "tblPr" | "tcPr" => result.styles.push(substyle(xml, child, "td", ignore_width)),
            "tblStylePr" => {
                result
                    .styles
                    .extend(
                        parse_table_style(xml, child, ignore_width)
                            .into_iter()
                            .map(|s| IDomSubStyle {
                                target: s.target,
                                r#mod: Some(s.r#mod),
                                values: s.values,
                            }),
                    );
            }
            _ => {}
        }
    }
    Ok(result)
}
