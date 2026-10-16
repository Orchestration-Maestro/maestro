//! Strict fixture decoding into the existing owned application records.
use indexmap::IndexMap;
use maestro_app::browser::docx_preview::{
    CommonProperties, IDomStyle, IDomSubStyle, LineSpacing, Numbering, ParagraphProperties, TabStop,
};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

/// Remove one encountered field without flattening a present null.
fn take<T: DeserializeOwned>(object: &mut Map<String, Value>, key: &str) -> Option<T> {
    object
        .shift_remove(key)
        .map(|v| serde_json::from_value(v).unwrap())
}
/// Reject fixture fields not consumed by an observation or operation.
fn complete(object: &Map<String, Value>) {
    assert!(
        object.is_empty(),
        "unconsumed fixture members: {:?}",
        object.keys()
    );
}
/// Decode a nullable scalar, represented identically when absent or null by its owner.
fn scalar(object: &mut Map<String, Value>, key: &str) -> Option<String> {
    take::<Option<String>>(object, key).flatten()
}
/// Decode common metadata while preserving encounter presence.
fn common(object: &mut Map<String, Value>) -> CommonProperties {
    CommonProperties {
        color: take(object, "color"),
        font_size: take(object, "fontSize"),
    }
}
/// Decode a complete common-property group.
fn run(value: Value) -> CommonProperties {
    let Value::Object(mut object) = value else {
        panic!("expected a fixture object")
    };
    let result = common(&mut object);
    complete(&object);
    result
}
/// Decode one tab record in its array position.
fn tab(value: Value) -> TabStop {
    let Value::Object(mut object) = value else {
        panic!("expected a fixture object")
    };
    let result = TabStop {
        position: scalar(&mut object, "position"),
        leader: scalar(&mut object, "leader"),
        style: scalar(&mut object, "style"),
    };
    complete(&object);
    result
}
/// Decode numbering with independently nullable fields.
fn numbering(value: Value) -> Numbering {
    let Value::Object(mut object) = value else {
        panic!("expected a fixture object")
    };
    let result = Numbering {
        id: take(&mut object, "id"),
        level: take(&mut object, "level"),
    };
    complete(&object);
    result
}
/// Decode line spacing; absent and null scalar values share the owner's representation.
fn spacing(value: Value) -> LineSpacing {
    let Value::Object(mut object) = value else {
        panic!("expected a fixture object")
    };
    let result = LineSpacing {
        before: scalar(&mut object, "before"),
        after: scalar(&mut object, "after"),
        line: take::<Option<f64>>(&mut object, "line").flatten(),
        line_rule: scalar(&mut object, "lineRule"),
    };
    complete(&object);
    result
}
/// Decode every property admitted by the style factory.
fn paragraph(value: Value) -> ParagraphProperties {
    let Value::Object(mut object) = value else {
        panic!("expected a fixture object")
    };
    let result = ParagraphProperties {
        common: common(&mut object),
        css_style: take(&mut object, "cssStyle"),
        tabs: take::<Vec<Value>>(&mut object, "tabs").map(|v| v.into_iter().map(tab).collect()),
        numbering: take(&mut object, "numbering").map(numbering),
        line_spacing: take(&mut object, "lineSpacing").map(spacing),
        text_alignment: take(&mut object, "textAlignment"),
        keep_lines: take(&mut object, "keepLines"),
        keep_next: take(&mut object, "keepNext"),
        page_break_before: take(&mut object, "pageBreakBefore"),
        outline_level: take(&mut object, "outlineLevel"),
        style_name: take(&mut object, "styleName"),
        run_props: take(&mut object, "runProps").map(run),
        class_name: scalar(&mut object, "className"),
    };
    complete(&object);
    result
}
/// Decode ordered declaration keys without JSON object equality hiding their order.
fn substyle(value: Value) -> IDomSubStyle {
    let Value::Object(mut object) = value else {
        panic!("expected a fixture object")
    };
    let result = IDomSubStyle {
        target: take(&mut object, "target").unwrap(),
        r#mod: scalar(&mut object, "mod"),
        values: take::<IndexMap<String, Option<String>>>(&mut object, "values").unwrap(),
    };
    complete(&object);
    result
}
/// Decode one complete style and its optional independently recorded CSS name.
pub(super) fn style(value: Value) -> (IDomStyle, Option<String>) {
    let Value::Object(mut object) = value else {
        panic!("expected a fixture object")
    };
    let css_name = scalar(&mut object, "cssName");
    let result = IDomStyle {
        id: scalar(&mut object, "id"),
        name: scalar(&mut object, "name"),
        target: scalar(&mut object, "target"),
        based_on: scalar(&mut object, "basedOn"),
        styles: take::<Vec<Value>>(&mut object, "styles")
            .unwrap()
            .into_iter()
            .map(substyle)
            .collect(),
        is_default: take(&mut object, "isDefault"),
        linked: take(&mut object, "linked"),
        next: take(&mut object, "next"),
        aliases: take(&mut object, "aliases"),
        paragraph_props: take(&mut object, "paragraphProps").map(paragraph),
        run_props: take(&mut object, "runProps").map(run),
    };
    complete(&object);
    (result, css_name)
}
