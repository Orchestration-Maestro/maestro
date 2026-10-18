//! Native browser observations through the private renderer.
use super::{
    HtmlRenderer,
    elements_fixture::{Input, cases},
};
use maestro_app::browser::docx_preview::{
    OpenXmlElementData as Data, OpenXmlElementTree, parse_body_elements, parse_xml_string,
};
use serde_json::{Value, json};
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{Document, Node};

/// Parse owned body content and release its XML input before rendering.
fn body(source: &str) -> OpenXmlElementTree {
    let source = format!(
        r#"<w:body xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">{source}</w:body>"#
    );
    let xml = parse_xml_string(&source, false).unwrap();
    parse_body_elements(&maestro_app::browser::docx_preview::OwnedXmlElement::new(
        xml.root(),
    ))
    .unwrap()
}

/// Compare the complete native tree and retained metadata for one behavior.
async fn check(name: &str) {
    let document = test_document();
    let selected = cases(name);
    assert!(!selected.is_empty());
    for (index, case) in selected.into_iter().enumerate() {
        let observed = match case.input {
            Input::Prepare { styles, levels } => {
                assert_eq!(case.outcome, "success");
                prepare(&document, &styles, &levels)
            }
            Input::Class {
                class_name,
                style_name,
            } => observe_class(
                &document,
                class_name.as_deref(),
                style_name.as_deref(),
                &case.outcome,
            ),
            Input::Style { values } => {
                assert_eq!(case.outcome, "success");
                let output = document.create_element("span").unwrap();
                HtmlRenderer::render_style_values(Some(&values.into_iter().collect()), &output)
                    .unwrap();
                node(output.as_ref())
            }
            Input::Elements {
                body: source,
                styles,
                repeat,
            } => {
                assert_eq!(case.outcome, "success");
                observe_elements(
                    &document,
                    &source,
                    styles.as_deref(),
                    repeat == Some(true),
                    (name, index),
                )
                .await
            }
            _ => unreachable!(),
        };
        assert_eq!(observed, case.expected, "{name} case {index}");
    }
}

/// Retain native failure category and the class assignment that preceded it.
fn observe_class(
    document: &Document,
    class_name: Option<&str>,
    style_name: Option<&str>,
    outcome: &str,
) -> Value {
    let output = document.create_element("span").unwrap();
    match HtmlRenderer::new("docx".into()).render_class(class_name, style_name, &output) {
        Ok(()) => {
            assert_eq!(outcome, "success");
            node(output.as_ref())
        }
        Err(error) => {
            assert_eq!(outcome, "error");
            json!({"name":error.dyn_ref::<web_sys::DomException>().unwrap().name(),"priorClass":output.class_name()})
        }
    }
}

/// Observe retained tabs, replay and native output before releasing mounted nodes.
async fn observe_elements(
    document: &Document,
    source: &str,
    styles: Option<&str>,
    repeat: bool,
    case: (&str, usize),
) -> Value {
    let mut tree = body(source);
    let roots = tree.roots.clone();
    let mut renderer = HtmlRenderer::new("docx".into());
    if let Some(styles) = styles {
        renderer.process_styles(parse_styles(styles));
    }
    let before = tabs(&tree);
    let nodes = renderer
        .render_elements(document, &mut tree, &roots)
        .unwrap();
    if repeat {
        let after = tabs(&tree);
        let second = renderer
            .render_elements(document, &mut tree, &roots)
            .unwrap();
        assert_eq!(
            nodes.iter().map(node).collect::<Vec<_>>(),
            second.iter().map(node).collect::<Vec<_>>()
        );
        assert_eq!(after, tabs(&tree));
    }
    let observed = json!({"nodes":nodes.iter().map(node).collect::<Vec<_>>(),"beforeTabs":before,"afterTabs":tabs(&tree)});
    observe_mounted(document, &renderer, &nodes, case).await;
    observed
}

/// Attach owned nodes for native geometry and foreground-paint observations.
async fn observe_mounted(
    document: &Document,
    renderer: &HtmlRenderer,
    nodes: &[Node],
    case: (&str, usize),
) {
    let (name, index) = case;
    let holder = document.create_element("div").unwrap();
    holder.set_id("docx-test");
    holder.set_class_name("docx");
    let style = document.create_element("style").unwrap();
    style.set_text_content(Some(&renderer.render_default_style(false)));
    holder.append_child(&style).unwrap();
    for node in nodes {
        holder.append_child(node).unwrap();
    }
    document.body().unwrap().append_child(&holder).unwrap();
    if name == "paragraph_common_values_override_converted_declarations" {
        let scene = if index == 0 {
            "docx-common-green"
        } else {
            "docx-common-red"
        };
        reading_paint::observe(scene).await;
    }
    if name == "nonbreaking_hyphens_preserve_a_visible_nonbreaking_character" {
        hyphen_geometry();
    }
    holder.remove();
}

/// Record ordered native attributes, children and text without flattening nodes.
fn node(input: &Node) -> Value {
    if input.node_type() == Node::TEXT_NODE {
        return json!({"text":input.node_value().unwrap()});
    }
    let element = input.dyn_ref::<web_sys::Element>().unwrap();
    let names = element.get_attribute_names();
    let attrs = names
        .iter()
        .map(|name| {
            let name = name.as_string().unwrap();
            let value = element.get_attribute(&name).unwrap();
            (name, value)
        })
        .collect::<Vec<_>>();
    let mut children = Vec::new();
    let mut child = input.first_child();
    while let Some(current) = child {
        children.push(node(&current));
        child = current.next_sibling();
    }
    json!({"tag":element.tag_name().to_lowercase(), "attrs":attrs, "children":children})
}

/// Project only actual paragraph tab metadata, retaining absent versus empty groups.
fn tabs(tree: &OpenXmlElementTree) -> Value {
    let mut pending = tree.roots.iter().rev().copied().collect::<Vec<_>>();
    let mut result = Vec::new();
    while let Some(index) = pending.pop() {
        let node = &tree.nodes[index];
        pending.extend(node.base.children.iter().rev().copied());
        if let Data::Paragraph { properties } = &node.data {
            result.push(properties.tabs.as_ref().map_or_else(
                || json!({"absent":true}),
                |tabs| {
                    json!(
                        tabs.iter()
                            .map(
                                |t| json!({"position":t.position,"leader":t.leader,"style":t.style})
                            )
                            .collect::<Vec<_>>()
                    )
                },
            ));
        }
    }
    json!(result)
}

/// Paragraphs keep empty and ordered native children.
#[wasm_bindgen]
pub async fn paragraphs_keep_empty_and_ordered_native_children() {
    check("paragraphs_keep_empty_and_ordered_native_children").await;
}

/// Text nodes preserve unicode whitespace and markup.
#[wasm_bindgen]
pub async fn text_nodes_preserve_unicode_whitespace_and_markup() {
    check("text_nodes_preserve_unicode_whitespace_and_markup").await;
}

/// Only text wrapping breaks emit line breaks.
#[wasm_bindgen]
pub async fn only_text_wrapping_breaks_emit_line_breaks() {
    check("only_text_wrapping_breaks_emit_line_breaks").await;
}

/// Tabs keep em space and nonbreaking hyphens keep visible text.
#[wasm_bindgen]
pub async fn tabs_keep_em_space_and_nonbreaking_hyphens_keep_visible_text() {
    check("tabs_keep_em_space_and_nonbreaking_hyphens_keep_visible_text").await;
}

/// Run alignment wraps children only for supported tags.
#[wasm_bindgen]
pub async fn run_alignment_wraps_children_only_for_supported_tags() {
    check("run_alignment_wraps_children_only_for_supported_tags").await;
}

/// Field runs are omitted without losing neighboring runs.
#[wasm_bindgen]
pub async fn field_runs_are_omitted_without_losing_neighboring_runs() {
    check("field_runs_are_omitted_without_losing_neighboring_runs").await;
}

/// Simple fields and markers do not emit unselected content.
#[wasm_bindgen]
pub async fn simple_fields_and_markers_do_not_emit_unselected_content() {
    check("simple_fields_and_markers_do_not_emit_unselected_content").await;
}

/// Selected change policy flattens insertions and omits deletions.
#[wasm_bindgen]
pub async fn selected_change_policy_flattens_insertions_and_omits_deletions() {
    check("selected_change_policy_flattens_insertions_and_omits_deletions").await;
}

/// Symbols follow native numeric entity and font semantics.
#[wasm_bindgen]
pub async fn symbols_follow_native_numeric_entity_and_font_semantics() {
    check("symbols_follow_native_numeric_entity_and_font_semantics").await;
}

/// Symbol attributes remain text while numeric entities keep native decoding.
#[wasm_bindgen]
pub fn symbol_attributes_never_create_elements_or_event_handlers() {
    let document = test_document();
    let renderer = HtmlRenderer::new("docx".into());
    for (value, expected) in [
        (
            "41;&lt;img src=x onerror=alert(1)&gt;",
            "A<img src=x onerror=alert(1)>;",
        ),
        ("41Z", "AZ;"),
        ("80", "€"),
        ("0", "\u{fffd}"),
        ("D800", "\u{fffd}"),
        ("FFFFFFFFFFFFFFFF", "\u{fffd}"),
        ("F0E0", "\u{f0e0}"),
        ("ZZZZ", "&#xZZZZ;"),
    ] {
        let mut tree = body(&format!(
            r#"<w:p><w:r><w:sym w:font="Wingdings" w:char="{value}"/></w:r></w:p>"#
        ));
        let roots = tree.roots.clone();
        let nodes = renderer
            .render_elements(&document, &mut tree, &roots)
            .unwrap();
        let symbol = nodes[0]
            .dyn_ref::<web_sys::Element>()
            .unwrap()
            .query_selector("span span")
            .unwrap()
            .unwrap();
        assert_eq!(symbol.child_element_count(), 0, "{value}");
        assert!(symbol.query_selector("[onerror]").unwrap().is_none());
        assert_eq!(symbol.text_content().as_deref(), Some(expected), "{value}");
        assert_eq!(
            symbol.get_attribute("style").as_deref(),
            Some("font-family: Wingdings;")
        );
    }
}

/// Paragraph common values override converted declarations.
#[wasm_bindgen]
pub async fn paragraph_common_values_override_converted_declarations() {
    check("paragraph_common_values_override_converted_declarations").await;
}

/// Parse retained styles through the existing document factory.
fn parse_styles(source: &str) -> Vec<maestro_app::browser::docx_preview::IDomStyle> {
    let source = format!(
        r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">{source}</w:styles>"#
    );
    super::parsed_styles(&source, false)
}

/// Paragraphs use style numbering only when local group is absent.
#[wasm_bindgen]
pub async fn paragraphs_use_style_numbering_only_when_local_group_is_absent() {
    check("paragraphs_use_style_numbering_only_when_local_group_is_absent").await;
}

/// Paragraph tabs inherit once but preserve explicit empty group.
#[wasm_bindgen]
pub async fn paragraph_tabs_inherit_once_but_preserve_explicit_empty_group() {
    check("paragraph_tabs_inherit_once_but_preserve_explicit_empty_group").await;
}

/// Conditional classes and converted style classes remain ordered.
#[wasm_bindgen]
pub async fn conditional_classes_and_converted_style_classes_remain_ordered() {
    check("conditional_classes_and_converted_style_classes_remain_ordered").await;
}

/// Element style values use native css and attribute assignment.
#[wasm_bindgen]
pub async fn element_style_values_use_native_css_and_attribute_assignment() {
    check("element_style_values_use_native_css_and_attribute_assignment").await;
}

/// Element classes keep assignment then style token order.
#[wasm_bindgen]
pub async fn element_classes_keep_assignment_then_style_token_order() {
    check("element_classes_keep_assignment_then_style_token_order").await;
}

/// Observe the retained occurrence, lookup and subsequent native paragraph together.
fn prepare(document: &Document, styles: &str, levels: &str) -> Value {
    let mut renderer = HtmlRenderer::new("docx".into());
    renderer.process_styles(parse_styles(styles));
    renderer.prodess_numberings(&super::elements_fixture::numberings(levels, None));
    let mut tree = body(
        r#"<w:p><w:pPr><w:pStyle w:val="List"/></w:pPr><w:r><w:t>list item</w:t></w:r></w:p>"#,
    );
    let roots = tree.roots.clone();
    let paragraph = renderer
        .render_elements(document, &mut tree, &roots)
        .unwrap();
    json!({"styles":renderer.styles.iter().map(|s| json!({"id":s.id,"numbering":numbering(s.paragraph_props.as_ref().and_then(|p| p.numbering.as_ref()))})).collect::<Vec<_>>(),
        "lookup":numbering(renderer.find_style(Some("List")).and_then(|s| s.paragraph_props.as_ref()).and_then(|p| p.numbering.as_ref())),
        "paragraph":node(&paragraph[0])})
}

/// Project numbering metadata without flattening missing scalar members.
fn numbering(input: Option<&maestro_app::browser::docx_preview::Numbering>) -> Value {
    let Some(input) = input else {
        return Value::Null;
    };
    let mut result = serde_json::Map::new();
    if let Some(id) = &input.id {
        result.insert("id".into(), json!(id));
    }
    if let Some(level) = input.level {
        result.insert(
            "level".into(),
            level.map_or(Value::Null, |value| {
                serde_json::from_str(ryu_js::Buffer::new().format(value)).unwrap()
            }),
        );
    }
    Value::Object(result)
}

/// Numbering preparation updates the retained last matching style.
#[wasm_bindgen]
pub async fn numbering_preparation_updates_the_retained_last_matching_style() {
    check("numbering_preparation_updates_the_retained_last_matching_style").await;
}

#[path = "../../../tests/support/reading_paint.rs"]
mod reading_paint;

/// Compare actual character line positions with both breakable controls.
fn hyphen_geometry() {
    let result = js_sys::eval(
        r"(() => {
        const actual = document.querySelector('#docx-test p');
        actual.style.cssText = 'width:150px;font:16px monospace;';
        const controls = ['prefix non<wbr>breaking c', 'prefix non-breaking c'].map(markup => {
            const p = document.createElement('p');
            p.style.cssText = actual.style.cssText;
            const span = document.createElement('span'); span.innerHTML = markup;
            p.appendChild(span); actual.parentElement.appendChild(p); return p;
        });
        const lines = [actual, ...controls].map((p, index) => {
            const walker = document.createTreeWalker(p, NodeFilter.SHOW_TEXT), tops = [];
            let node;
            while (node = walker.nextNode()) for (let i=0; i<node.length; i++) {
                const range = document.createRange();range.setStart(node,i);range.setEnd(node,i+1);
                tops.push(range.getBoundingClientRect().top);
            }
            return new Set(tops.slice(7,index===1?18:19)).size;
        });
        controls.forEach(p => p.remove());
        return JSON.stringify(lines);
    })()",
    )
    .unwrap();
    assert_eq!(result.as_string().unwrap(), "[1,2,2]");
}

/// Nonbreaking hyphens preserve a visible nonbreaking character.
#[wasm_bindgen]
pub async fn nonbreaking_hyphens_preserve_a_visible_nonbreaking_character() {
    check("nonbreaking_hyphens_preserve_a_visible_nonbreaking_character").await;
}

/// Generated markers retain literal paint and layout around genuine counters.
#[wasm_bindgen]
pub async fn numbering_literals_reach_native_content_without_css_injection() {
    let document = test_document();
    let holder = document.create_element("div").unwrap();
    holder.set_id("docx-literals");
    let renderer = HtmlRenderer::new("docx".into());
    for (index, (text, display)) in [
        ("Section \"%1\"", "Section \"7\""),
        ("C:\\%1", "C:\\7"),
        ("line\n%1", "line\n7"),
        ("100% done", "100% done"),
        ("%%1", "%7"),
        ("<%1>", "<7>"),
    ]
    .into_iter()
    .enumerate()
    {
        let content = renderer.level_text_to_content(
            text,
            None,
            maestro_app::Presence::Present("11"),
            maestro_app::Presence::Present("decimal"),
        );
        let style = document.create_element("style").unwrap();
        style.set_text_content(Some(&format!(
            "#docx-literal-{index}::before{{content:{content}}}"
        )));
        holder.append_child(&style).unwrap();
        for (kind, literal) in [("literal", None), ("control", Some(display))] {
            let output = document.create_element("p").unwrap();
            output.set_id(&format!("docx-{kind}-{index}"));
            output.set_attribute("style", "display:inline-block;margin:0;font:16px monospace;white-space:pre;counter-reset:docx-num-11-0 7;").unwrap();
            output.set_text_content(literal);
            holder.append_child(&output).unwrap();
        }
    }
    document.body().unwrap().append_child(&holder).unwrap();
    reading_paint::observe("docx-literals").await;
    holder.remove();
}

/// Install browser failure diagnostics before obtaining the actual native document.
fn test_document() -> Document {
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&info.to_string().into());
    }));
    web_sys::window().unwrap().document().unwrap()
}
