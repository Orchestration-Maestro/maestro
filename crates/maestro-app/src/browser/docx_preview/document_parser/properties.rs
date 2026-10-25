//! Borrowed property interception over the shared CSS dispatcher.
use super::super::document::paragraph::parse_paragraph_property;
use super::super::{OpenXmlElementData as Data, XmlElement, XmlParser};
use super::{DefaultPropertiesOptions, parse_default_properties};

/// Replace paragraph CSS while retaining previously encountered metadata.
pub(super) fn parse_paragraph_properties<'doc, 'input: 'doc>(
    xml: &'doc XmlParser<'input>,
    element: XmlElement<'doc, 'input>,
    data: &mut Data,
    ignore_width: bool,
) {
    let Data::Paragraph { properties: props } = data else {
        return;
    };
    let mut style = indexmap::IndexMap::new();
    let mut handler =
        |child: XmlElement<'doc, 'input>,
         style: &mut indexmap::IndexMap<String, Option<String>>| {
            if parse_paragraph_property(xml, child, props) {
                return true;
            }
            match child.local_name() {
                "pStyle" => props.style_name = Some(xml.attr(child, "val").map(str::to_owned)),
                "framePr" => parse_frame(xml, child, style),
                "rPr" => {}
                "cnfStyle" => {
                    props.class_name = Some(super::values::class_name_of_cnf_style(xml, child));
                }
                _ => return false,
            }
            true
        };
    parse_default_properties(
        xml,
        element,
        &mut style,
        None,
        DefaultPropertiesOptions {
            ignore_width,
            handler: Some(&mut handler),
        },
    );
    props.css_style = Some(style);
}
/// Replace run CSS from one immediate property group.
pub(super) fn parse_run_properties<'doc, 'input: 'doc>(
    xml: &'doc XmlParser<'input>,
    element: XmlElement<'doc, 'input>,
    data: &mut Data,
    ignore_width: bool,
) {
    let Data::Run {
        properties: props, ..
    } = data
    else {
        return;
    };
    let mut style = indexmap::IndexMap::new();
    let mut handler = |child: XmlElement<'doc, 'input>,
                       _: &mut indexmap::IndexMap<String, Option<String>>| {
        match child.local_name() {
            "rStyle" => props.style_name = Some(xml.attr(child, "val").map(str::to_owned)),
            "vertAlign" => {
                props.vertical_align = Some(super::values::value_of_vert_align(xml, child));
            }
            _ => return false,
        }
        true
    };
    parse_default_properties(
        xml,
        element,
        &mut style,
        None,
        DefaultPropertiesOptions {
            ignore_width,
            handler: Some(&mut handler),
        },
    );
    props.css_style = Some(style);
}

/// Apply floating drop-cap frames to the actual dispatched style map.
fn parse_frame(
    xml: &XmlParser<'_>,
    element: XmlElement<'_, '_>,
    style: &mut indexmap::IndexMap<String, Option<String>>,
) {
    if xml.attr(element, "dropCap") == Some("drop") {
        style.insert("float".into(), Some("left".into()));
    }
}
