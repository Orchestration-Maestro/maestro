//! Element text, attribute and declaration serialization, and style lookup.
use super::{HtmlElement, check, render};
use crate::docx_preview::html_renderer::Styles;
use maestro_app::browser::docx_preview::{StylesPart, parse_xml_string};

#[test]
fn hostile_text_and_attribute_values_stay_literal() {
    check("inline-hostile-values");
    assert_eq!(
        render("inline-hostile-values")[0].to_html(),
        "<section class=\"docx\"><article><p><span>&lt;tag&gt; &amp; &quot;quoted&quot; \
         &#39;single&#39;</span><span id=\"a&quot;b&amp;c\"></span><span>\
         <span style=\"font-family:&quot;Quo\\&quot;te&amp;&quot;\">A</span></span>\
         </p></article></section>"
    );
}

#[test]
fn text_and_attribute_values_are_escaped() {
    let mut element = HtmlElement::new("span");
    element.set_attribute("title", "x\" onmouseover=\"alert(1)");
    element.set_style("font-family", Some("'Times \"New\" Roman'"));
    element.set_style("color", Some("red;position:fixed"));
    element.set_style("width", Some("1pt/*"));
    element.set_style("height", Some("1pt}"));
    element.set_style("margin-left", Some("1pt !important"));
    element.set_style("margin-top", Some("calc(1pt"));
    element.set_style("margin-right", Some("'1pt"));
    element.set_style("margin-bottom", Some("\"1pt\\\""));
    element.set_style("font-style", Some("\"a\nb\""));
    element.set_style("font-weight", Some("1pt)"));
    element.push_text("</span><script>alert('&amp;')</script>");
    let mut child = HtmlElement::new("br");
    child.set_attribute("id", "<br>");
    element.push_element(child);
    assert_eq!(
        element.to_html(),
        "<span title=\"x&quot; onmouseover=&quot;alert(1)\" \
         style=\"font-family:&#39;Times &quot;New&quot; Roman&#39;\">\
         &lt;/span&gt;&lt;script&gt;alert(&#39;&amp;amp;&#39;)&lt;/script&gt;\
         <br id=\"&lt;br&gt;\"></span>"
    );
}

#[test]
fn style_declarations_are_sorted_and_keep_the_last_value() {
    let mut element = HtmlElement::new("p");
    element.set_style("width", Some("1pt"));
    element.set_style("color", Some("red"));
    element.set_style("background-color", Some("#EEFFEE"));
    element.set_style("width", Some("2pt"));
    element.set_style("height", Some("3pt"));
    element.set_style("height", None);
    element.set_style("color", Some(""));
    element.set_style("border", Some("none"));
    assert_eq!(
        element.to_html(),
        "<p style=\"background-color:#EEFFEE;border:none;width:2pt\"></p>"
    );
    element.set_style("border", None);
    element.set_style("background-color", None);
    element.set_style("width", None);
    assert_eq!(element.to_html(), "<p></p>");
}

/// Page and column geometry is assigned through camel-case style properties.
#[test]
fn style_values_convert_camel_case_names_to_css_properties() {
    let html = render("columns")[0].to_html();
    assert_eq!(
        html,
        "<section class=\"docx\" style=\"min-height:841.90pt;padding-bottom:72.00pt;\
         padding-left:72.00pt;padding-right:72.00pt;padding-top:72.00pt;width:595.30pt\">\
         <article style=\"column-count:2;column-gap:36.00pt;column-rule:1px solid black\">\
         <p><span>one</span></p></article></section>"
    );
}

#[test]
fn style_inheritance_follows_each_base_and_keeps_last_duplicates() {
    let source = r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:style w:type="paragraph" w:styleId="Child"><w:basedOn w:val="Middle"/><w:pPr><w:keepNext/></w:pPr></w:style>
<w:style w:type="paragraph" w:styleId="Middle"><w:basedOn w:val="Base"/><w:pPr><w:pageBreakBefore/></w:pPr></w:style>
<w:style w:type="paragraph" w:styleId="Base"><w:pPr><w:numPr><w:numId w:val="7"/><w:ilvl w:val="2"/></w:numPr></w:pPr></w:style>
<w:style w:type="paragraph" w:styleId="Twin"><w:pPr><w:numPr><w:numId w:val="1"/></w:numPr></w:pPr></w:style>
<w:style w:type="paragraph" w:styleId="Twin"><w:pPr><w:numPr><w:numId w:val="2"/></w:numPr></w:pPr></w:style>
<w:style w:type="paragraph" w:styleId="Orphan"><w:basedOn w:val="Missing"/><w:pPr><w:numPr><w:numId w:val="3"/></w:numPr></w:pPr></w:style>
</w:styles>"#;
    let xml = parse_xml_string(source, false).unwrap();
    let parsed = StylesPart::parse_xml(&xml, xml.root(), false).unwrap();
    let styles = Styles::process_styles(&parsed.styles);
    let numbering = |name| {
        let props = styles
            .find_style(Some(name))
            .unwrap()
            .paragraph_props
            .as_ref();
        let numbering = props.unwrap().numbering.as_ref().unwrap();
        (numbering.id.clone().flatten(), numbering.level.flatten())
    };
    let child = styles.find_style(Some("Child")).unwrap();
    let child_props = child.paragraph_props.as_ref().unwrap();
    assert_eq!(
        (child_props.keep_next, child_props.page_break_before),
        (Some(true), Some(true))
    );
    assert_eq!(numbering("Child"), (Some("7".into()), Some(2.0)));
    assert_eq!(numbering("Twin"), (Some("2".into()), None));
    assert_eq!(numbering("Orphan"), (Some("3".into()), None));
    let order: Vec<_> = styles
        .styles
        .iter()
        .map(|style| style.id.as_deref().unwrap())
        .collect();
    assert_eq!(order, ["Child", "Middle", "Base", "Twin", "Twin", "Orphan"]);
    assert!(styles.find_style(Some("")).is_none() && styles.find_style(None).is_none());
}
