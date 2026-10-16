//! Lossless observations of owned Word trees.
#![cfg(test)]
use maestro_app::browser::docx_preview::{
    FooterHeaderReference, OpenXmlElementData as Data, OpenXmlElementTree, OwnedXmlElement,
};
use serde_json::{Value, json};

/// Observe the retained XML selection, including mixed element/text order.
///
/// # Panics
/// Panics if the retained source cannot be reparsed or its range is missing.
#[must_use]
pub fn raw(input: &OwnedXmlElement) -> Value {
    let parser = roxmltree::Document::parse(input.source()).unwrap();
    let element = parser
        .descendants()
        .find(|n| n.is_element() && n.range() == input.source_range())
        .unwrap();
    xml(element)
}
/// Project one XML element without discarding text or namespace identity.
fn xml(element: roxmltree::Node<'_, '_>) -> Value {
    json!({
        "local_name": element.tag_name().name(), "namespace": element.tag_name().namespace(),
        "attributes": element.attributes().map(|a| json!({"local_name": a.name(), "namespace": a.namespace(), "value": a.value()})).collect::<Vec<_>>(),
        "children": element.children().filter_map(|n| if n.is_element() { Some(xml(n)) } else { n.text().map(|t| json!(t)) }).collect::<Vec<_>>()
    })
}
/// Scalar payload observation; common links are observed separately.
fn payload(data: &Data) -> Value {
    match data {
        Data::Run {
            field_run: true, ..
        } => json!({"fieldRun":true}),
        Data::Text { text } | Data::DeletedText { text } | Data::Instruction { text } => {
            json!({"text":text})
        }
        Data::BookmarkStart {
            id,
            name,
            col_first,
            col_last,
        } => {
            json!({"id":id,"name":name,"colFirst":col_first.map_or(Value::Null,number),"colLast":col_last.map_or(Value::Null,number)})
        }
        Data::BookmarkEnd { id } => json!({"id":id}),
        Data::Break { r#break } => json!({"break":r#break}),
        Data::Symbol { font, char } => json!({"font":font,"char":char}),
        Data::FootnoteReference { id } | Data::EndnoteReference { id } | Data::AltChunk { id } => {
            json!({"id":id})
        }
        Data::SimpleField {
            instruction,
            lock,
            dirty,
        } => json!({"instruction":instruction,"lock":lock,"dirty":dirty}),
        Data::ComplexField {
            char_type,
            lock,
            dirty,
        } => json!({"charType":char_type,"lock":lock,"dirty":dirty}),
        _ => json!({}),
    }
}
/// Assign graph positions by children first, then parent-only identities.
fn visit(
    tree: &OpenXmlElementTree,
    id: usize,
    positions: &mut [Option<usize>],
    nodes: &mut Vec<Value>,
    observe: fn(&Data, &mut Value),
) -> usize {
    if let Some(index) = positions[id] {
        return index;
    }
    let index = nodes.len();
    positions[id] = Some(index);
    nodes.push(Value::Null);
    let node = &tree.nodes[id];
    let children: Vec<_> = node
        .base
        .children
        .iter()
        .map(|&child| visit(tree, child, positions, nodes, observe))
        .collect();
    let parent = node
        .base
        .parent
        .map(|parent| visit(tree, parent, positions, nodes, observe));
    let mut value = payload(&node.data);
    value["type"] = json!(node.dom_type().as_str());
    if matches!(
        node.data,
        Data::Paragraph { .. }
            | Data::Run { .. }
            | Data::SimpleField { .. }
            | Data::Inserted
            | Data::Deleted
            | Data::AltChunk { .. }
            | Data::Drawing { .. }
            | Data::Math { .. }
            | Data::VmlPicture
            | Data::VmlElement(_)
    ) {
        value["children"] = json!(children);
    } else {
        assert!(children.is_empty());
    }
    if matches!(node.data, Data::Run { .. }) {
        value["parent"] = json!(parent);
    } else {
        assert!(parent.is_none());
    }
    observe(&node.data, &mut value);
    nodes[index] = value;
    index
}
/// Observe the complete graph, including repeated links and hidden parents.
#[must_use]
pub fn graph(tree: &OpenXmlElementTree) -> Value {
    project(tree, |_, _| {})
}
/// Observe complete formatting on the same arena traversal.
#[must_use]
pub fn full_graph(tree: &OpenXmlElementTree) -> Value {
    project(tree, formatting)
}
/// Traverse with the chosen payload observation.
fn project(tree: &OpenXmlElementTree, observe: fn(&Data, &mut Value)) -> Value {
    let mut positions = vec![None; tree.nodes.len()];
    let mut nodes = Vec::new();
    let roots: Vec<_> = tree
        .roots
        .iter()
        .map(|&root| visit(tree, root, &mut positions, &mut nodes, observe))
        .collect();
    json!({"roots":roots,"nodes":nodes})
}
/// Observe every foreign slot with its actual kind and complete retained XML.
#[must_use]
pub fn foreign(tree: &OpenXmlElementTree, context: &str) -> Value {
    json!(tree.nodes.iter().filter_map(|node| if let Data::Unparsed { element, .. } = &node.data {
        let xml = raw(element);
        Some(json!({"context":context,"name":xml["local_name"],"kind":node.dom_type().as_str(),"xml":xml}))
    } else { None }).collect::<Vec<_>>())
}

/// Add an encountered member without conflating absent and null.
fn member<T>(out: &mut Value, key: &str, value: Option<T>, encode: impl FnOnce(T) -> Value) {
    if let Some(value) = value {
        out[key] = encode(value);
    }
}
/// Preserve nonfinite numbers and negative zero as fixture-only markers.
fn number(value: f64) -> Value {
    if value.is_nan() {
        json!({"$number":"NaN"})
    } else if value == 0.0 && value.is_sign_negative() {
        json!({"$number":"-0"})
    } else {
        serde_json::from_str(ryu_js::Buffer::new().format(value)).unwrap()
    }
}
/// Common raw metadata uses the same presence rules in either owner.
pub(super) fn common(
    props: &maestro_app::browser::docx_preview::CommonProperties,
    out: &mut Value,
) {
    member(out, "color", props.color.as_ref(), |v| json!(v));
    member(out, "fontSize", props.font_size.as_ref(), |v| json!(v));
}
/// Observe every owned property, ordered styles and nullable record members.
pub(super) fn formatting(data: &Data, out: &mut Value) {
    match data {
        Data::Paragraph { properties: p } => paragraph_properties(p, out),
        Data::Image {
            src,
            css_style,
            src_rect,
            rotation,
        } => {
            out["src"] = json!(src);
            styles(Some(css_style), out);
            member(out, "srcRect", *src_rect, |v| json!(v.map(number)));
            member(out, "rotation", *rotation, number);
        }
        Data::Math { props, .. } => member(out, "props", props.as_ref(), |p| {
            let mut v = json!({});
            member(&mut v, "char", p.char.as_ref(), |v| json!(v));
            member(
                &mut v,
                "verticalJustification",
                p.vertical_justification.as_ref(),
                |v| json!(v),
            );
            member(&mut v, "position", p.position.as_ref(), |v| json!(v));
            member(&mut v, "hideDegree", p.hide_degree, |v| json!(v));
            member(&mut v, "beginChar", p.begin_char.as_ref(), |v| json!(v));
            member(&mut v, "endChar", p.end_char.as_ref(), |v| json!(v));
            v
        }),
        Data::VmlElement(v) => vml(v, out),
        Data::Drawing { css_style } => styles(Some(css_style), out),
        Data::Run { properties: p, .. } => {
            styles(p.css_style.as_ref(), out);
            member(out, "styleName", p.style_name.as_ref(), |v| json!(v));
            member(out, "verticalAlign", p.vertical_align.as_ref(), |v| {
                json!(v.as_ref().map(|v| match v {
                    maestro_app::browser::docx_preview::VerticalAlign::Sub => "sub",
                    maestro_app::browser::docx_preview::VerticalAlign::Sup => "sup",
                }))
            });
        }
        _ => {}
    }
}
/// Style insertion order is part of the observation, not JSON map equality.
fn styles(style: Option<&indexmap::IndexMap<String, Option<String>>>, out: &mut Value) {
    member(out, "cssStyle", style, |v| {
        json!(v.iter().collect::<Vec<_>>())
    });
}

/// Observe paragraph metadata through the same graph projection.
fn paragraph_properties(
    p: &maestro_app::browser::docx_preview::ParagraphProperties,
    out: &mut Value,
) {
    common(&p.common, out);
    member(out, "sectionProps", p.section_props.as_ref(), section);
    styles(p.css_style.as_ref(), out);
    member(out, "tabs", p.tabs.as_ref(), |tabs| {
        json!(
            tabs.iter()
                .map(|t| json!({"position":t.position,"leader":t.leader,"style":t.style}))
                .collect::<Vec<_>>()
        )
    });
    member(out, "numbering", p.numbering.as_ref(), |n| {
        let mut value = json!({});
        member(&mut value, "id", n.id.as_ref(), |v| json!(v));
        member(&mut value, "level", n.level, |v| {
            v.map_or(Value::Null, number)
        });
        value
    });
    member(
        out,
        "lineSpacing",
        p.line_spacing.as_ref(),
        |l| json!({"before":l.before,"after":l.after,"line":l.line.map_or(Value::Null,number),"lineRule":l.line_rule}),
    );
    member(out, "textAlignment", p.text_alignment.as_ref(), |v| {
        json!(v)
    });
    member(out, "keepLines", p.keep_lines, |v| json!(v));
    member(out, "keepNext", p.keep_next, |v| json!(v));
    member(out, "pageBreakBefore", p.page_break_before, |v| json!(v));
    member(out, "outlineLevel", p.outline_level, |v| {
        v.map_or(Value::Null, number)
    });
    member(out, "styleName", p.style_name.as_ref(), |v| json!(v));
    member(out, "runProps", p.run_props.as_ref(), |p| {
        let mut v = json!({});
        common(p, &mut v);
        v
    });
    member(out, "className", p.class_name.as_ref(), |v| json!(v));
}

/// Observe ordered SVG attributes including unavailable coordinates.
fn vml(v: &maestro_app::browser::docx_preview::VmlElement, out: &mut Value) {
    use maestro_app::browser::docx_preview::VmlAttributeValue;
    styles(Some(&v.css_style), out);
    out["tagName"] = json!(v.tag_name.as_str());
    out["attrs"] = json!(
        v.attrs
            .iter()
            .map(|(k, v)| (
                k,
                match v {
                    VmlAttributeValue::Text(v) => json!(v),
                    VmlAttributeValue::Null => Value::Null,
                    VmlAttributeValue::Undefined => json!({"$undefined":true}),
                }
            ))
            .collect::<Vec<_>>()
    );
    member(out, "cssStyleText", v.css_style_text.as_ref(), |v| json!(v));
    member(
        out,
        "imageHref",
        v.image_href.as_ref(),
        |v| json!({"id":v.id,"title":v.title}),
    );
}

/// Observe every section group without collapsing absent and nullable fields.
fn section(p: &maestro_app::browser::docx_preview::SectionProperties) -> Value {
    let mut out = json!({});
    member(
        &mut out,
        "pageSize",
        p.page_size.as_ref(),
        |s| json!({"width":s.width,"height":s.height,"orientation":s.orientation}),
    );
    member(&mut out, "pageMargins", p.page_margins.as_ref(), |m| {
        json!({"left":m.left,"right":m.right,"top":m.top,"bottom":m.bottom,
            "header":m.header,"footer":m.footer,"gutter":m.gutter})
    });
    member(&mut out, "type", p.r#type.as_ref(), |v| json!(v));
    member(&mut out, "columns", p.columns.as_ref(), |c| {
        json!({"numberOfColumns":c.number_of_columns.map_or(Value::Null,number),
            "space":c.space,"separator":c.separator,"equalWidth":c.equal_width,
            "columns":c.columns.iter().map(|c| json!({"width":c.width,"space":c.space})).collect::<Vec<_>>()})
    });
    let references = |refs: &Vec<FooterHeaderReference>| {
        json!(
            refs.iter()
                .map(|r| json!({"id":r.id,"type":r.r#type}))
                .collect::<Vec<_>>()
        )
    };
    member(&mut out, "headerRefs", p.header_refs.as_ref(), references);
    member(&mut out, "footerRefs", p.footer_refs.as_ref(), references);
    member(&mut out, "titlePage", p.title_page, |v| json!(v));
    member(&mut out, "pageNumber", p.page_number.as_ref(), |n| {
        json!({"chapSep":n.chap_sep,"chapStyle":n.chap_style,"format":n.format,
            "start":n.start.map_or(Value::Null,number)})
    });
    member(&mut out, "pageBorders", p.page_borders.as_ref(), |b| {
        let mut borders = json!({});
        for (key, side) in [
            ("left", &b.left),
            ("top", &b.top),
            ("right", &b.right),
            ("bottom", &b.bottom),
        ] {
            member(&mut borders, key, side.as_ref(), |s| {
                json!({"type":s.r#type,"color":s.color,"size":s.size,"offset":s.offset,
                    "frame":s.frame,"shadow":s.shadow})
            });
        }
        borders
    });
    out
}
