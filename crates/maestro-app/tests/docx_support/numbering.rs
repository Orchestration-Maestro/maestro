//! Complete owned numbering observations.
#![cfg(test)]
use crate::body_tree::{common, member, number, paragraph_properties};
use maestro_app::browser::docx_preview::{
    IDomNumbering, NumberingPicBullet,
    numbering::{AbstractNumbering, NumberingLevel, NumberingLevelOverride, NumberingPart},
};
use serde_json::{Value, json};
/// Encode nullable numeric attributes without losing signed zero or NaN.
fn numeric(value: Option<f64>) -> Value {
    value.map_or(Value::Null, number)
}
/// Observe an owned raw level, including encountered null fields.
fn level(level: &NumberingLevel) -> Value {
    let mut out = json!({"level":numeric(level.level)});
    member(&mut out, "start", level.start.as_ref(), |v| json!(v));
    member(&mut out, "restart", level.restart, numeric);
    member(&mut out, "format", level.format.as_ref(), |v| json!(v));
    member(&mut out, "text", level.text.as_ref(), |v| json!(v));
    member(
        &mut out,
        "justification",
        level.justification.as_ref(),
        |v| json!(v),
    );
    member(
        &mut out,
        "bulletPictureId",
        level.bullet_picture_id.as_ref(),
        |v| json!(v),
    );
    member(
        &mut out,
        "paragraphStyle",
        level.paragraph_style.as_ref(),
        |v| json!(v),
    );
    member(
        &mut out,
        "paragraphProps",
        level.paragraph_props.as_ref(),
        |v| {
            let mut out = json!({});
            paragraph_properties(v, &mut out);
            out
        },
    );
    member(&mut out, "runProps", level.run_props.as_ref(), |v| {
        let mut out = json!({});
        common(v, &mut out);
        out
    });
    out
}
/// Observe raw abstract metadata.
fn abstract_numbering(raw: &AbstractNumbering) -> Value {
    let mut out = json!({"id":raw.id,"levels":raw.levels.iter().map(level).collect::<Vec<_>>()});
    member(&mut out, "name", raw.name.as_ref(), |v| json!(v));
    member(
        &mut out,
        "multiLevelType",
        raw.multi_level_type.as_ref(),
        |v| json!(v),
    );
    member(
        &mut out,
        "numberingStyleLink",
        raw.numbering_style_link.as_ref(),
        |v| json!(v),
    );
    member(&mut out, "styleLink", raw.style_link.as_ref(), |v| json!(v));
    out
}
/// Observe each ordered override independently from the concrete view.
fn level_override(raw: &NumberingLevelOverride) -> Value {
    let mut out = json!({"level":numeric(raw.level)});
    member(&mut out, "start", raw.start, numeric);
    member(
        &mut out,
        "numberingLevel",
        raw.numbering_level.as_ref(),
        level,
    );
    out
}
/// Preserve the undefined identifier distinct from a null identifier.
fn undefined(value: Option<&Option<String>>) -> Value {
    value.map_or_else(|| json!({"$undefined":true}), |v| json!(v))
}
/// Observe a prepared picture, retaining nullable fields.
fn picture(value: &NumberingPicBullet) -> Value {
    json!({"id":numeric(value.id),"src":value.src,"style":value.style})
}
/// Observe complete presentation records and exact CSS key order.
fn dom(value: &IDomNumbering) -> Value {
    let mut out = json!({"id":undefined(value.id.as_ref()),"level":numeric(value.level),"start":numeric(value.start),"pStyleName":undefined(value.p_style_name.as_ref()),"pStyle":value.p_style.iter().collect::<Vec<_>>(),"rStyle":value.r_style.iter().collect::<Vec<_>>(),"suff":value.suff});
    member(&mut out, "levelText", value.level_text.as_ref(), |v| {
        json!(v)
    });
    member(&mut out, "format", value.format.as_ref(), |v| json!(v));
    member(&mut out, "bullet", value.bullet.as_ref(), |v| {
        v.as_ref()
            .map_or_else(|| json!({"$undefined":true}), picture)
    });
    out
}
/// Observe all three raw arrays and the resolved concrete sequence.
pub(super) fn observe(part: &NumberingPart) -> Value {
    json!({"raw":{
        "numberings":part.numberings.iter().map(|n| {let mut out=json!({"id":n.id,"overrides":n.overrides.iter().map(level_override).collect::<Vec<_>>()});member(&mut out,"abstractId",n.abstract_id.as_ref(),|v|json!(v));out}).collect::<Vec<_>>(),
        "abstractNumberings":part.abstract_numberings.iter().map(abstract_numbering).collect::<Vec<_>>(),
        "bulletPictures":part.bullet_pictures.iter().map(|p|p.as_ref().map(|p|json!({"id":p.id,"referenceId":p.reference_id,"style":p.style}))).collect::<Vec<_>>()},
        "dom":part.dom_numberings.iter().map(dom).collect::<Vec<_>>()})
}
