//! Lossless typed metadata observations.
#![cfg(test)]
use maestro_app::browser::docx_preview::CorePropsDeclaration;
use serde_json::{Value, json};
/// Observe selected core fields without inventing missing members.
#[must_use]
pub fn core(props: &CorePropsDeclaration) -> Value {
    let mut out = json!({});
    if let Some(value) = &props.title {
        out["title"] = json!(value);
    }
    if let Some(value) = &props.description {
        out["description"] = json!(value);
    }
    if let Some(value) = &props.subject {
        out["subject"] = json!(value);
    }
    if let Some(value) = &props.creator {
        out["creator"] = json!(value);
    }
    if let Some(value) = &props.keywords {
        out["keywords"] = json!(value);
    }
    if let Some(value) = &props.language {
        out["language"] = json!(value);
    }
    if let Some(value) = &props.last_modified_by {
        out["lastModifiedBy"] = json!(value);
    }
    if let Some(value) = props.revision {
        out["revision"] = super::body_tree::number(value);
    }
    out
}

/// Observe selected extended properties.
#[must_use]
pub fn extended(props: &maestro_app::browser::docx_preview::ExtendedPropsDeclaration) -> Value {
    let mut out = json!({});
    if let Some(value) = &props.template {
        out["template"] = json!(value);
    }
    if let Some(value) = &props.pages {
        out["pages"] = super::body_tree::number(*value);
    }
    if let Some(value) = &props.words {
        out["words"] = super::body_tree::number(*value);
    }
    if let Some(value) = &props.characters {
        out["characters"] = super::body_tree::number(*value);
    }
    if let Some(value) = &props.application {
        out["application"] = json!(value);
    }
    if let Some(value) = &props.lines {
        out["lines"] = super::body_tree::number(*value);
    }
    if let Some(value) = &props.paragraphs {
        out["paragraphs"] = super::body_tree::number(*value);
    }
    if let Some(value) = &props.company {
        out["company"] = json!(value);
    }
    if let Some(value) = &props.app_version {
        out["appVersion"] = json!(value);
    }
    out
}

/// Observe raw custom values in source order.
#[must_use]
pub fn custom(props: &[maestro_app::browser::docx_preview::CustomProperty]) -> Value {
    json!(
        props
            .iter()
            .map(|p| json!({"formatId":p.format_id,"name":p.name,"type":p.r#type,"value":p.value}))
            .collect::<Vec<_>>()
    )
}

/// Observe one complete comment, including its existing body observation.
#[must_use]
pub fn comment(c: &maestro_app::browser::docx_preview::WmlComment) -> Value {
    json!({"cssStyle":c.css_style.iter().collect::<Vec<_>>(),"type":"comment","id":c.id,"author":c.author,"initials":c.initials,"date":c.date,"children":super::body_tree::full_graph(&c.children)})
}
/// Observe source order and lookup identity rather than comparing cloned values.
fn lookup<T>(
    list: &[std::sync::Arc<std::sync::RwLock<T>>],
    map: &indexmap::IndexMap<String, std::sync::Arc<std::sync::RwLock<T>>>,
) -> Value {
    json!(map.iter().map(|(key, c)| json!({"key":key,"index":list.iter().position(|v| std::sync::Arc::ptr_eq(c,v)).unwrap()})).collect::<Vec<_>>())
}
/// Observe comments and the indices selected by each lookup key.
///
/// # Panics
/// Panics if a record is poisoned or a lookup does not alias the list.
#[must_use]
pub fn comments(part: &maestro_app::browser::docx_preview::CommentsPart) -> Value {
    json!({"comments":part.comments.iter().map(|c| comment(&c.read().unwrap())).collect::<Vec<_>>(),"lookup":lookup(&part.comments,&part.comment_map)})
}
/// Observe nullable links and lookup indices.
///
/// # Panics
/// Panics if a record is poisoned or a lookup does not alias the list.
#[must_use]
pub fn extensions(part: &maestro_app::browser::docx_preview::CommentsExtendedPart) -> Value {
    json!({"comments":part.comments.iter().map(|c| {let c=c.read().unwrap(); json!({"paraId":c.para_id,"paraIdParent":c.para_id_parent,"done":c.done})}).collect::<Vec<_>>(),"lookup":lookup(&part.comments,&part.comment_map)})
}

/// Observe the retained semantic parts and named borrowed accessors.
///
/// # Panics
/// Panics if a shared comment record is poisoned or its lookup is inconsistent.
#[must_use]
pub fn package(doc: &maestro_app::browser::docx_preview::WordDocument) -> Value {
    use maestro_app::browser::docx_preview::PartData;
    let mut selected = json!({"corePropsPart":null,"extendedPropsPart":null,"commentsPart":null,"commentsExtendedPart":null});
    let parts: Vec<_> = doc
        .parts()
        .iter()
        .filter_map(|p| {
            let (kind, data, chosen) = match p.data.as_ref()? {
                PartData::CoreProps(v) => (
                    "CorePropsPart",
                    core(&v.props),
                    doc.core_props_part()
                        .is_some_and(|c| std::ptr::eq(c, v.as_ref())),
                ),
                PartData::ExtendedProps(v) => (
                    "ExtendedPropsPart",
                    extended(&v.props),
                    doc.extended_props_part()
                        .is_some_and(|c| std::ptr::eq(c, v.as_ref())),
                ),
                PartData::CustomProps(v) => ("CustomPropsPart", custom(&v.props), false),
                PartData::Comments(v) => (
                    "CommentsPart",
                    comments(v),
                    doc.comments_part()
                        .is_some_and(|c| std::ptr::eq(c, v.as_ref())),
                ),
                PartData::CommentsExtended(v) => (
                    "CommentsExtendedPart",
                    extensions(v),
                    doc.comments_extended_part()
                        .is_some_and(|c| std::ptr::eq(c, v.as_ref())),
                ),
            };
            if chosen {
                let name = match p.data.as_ref()? {
                    PartData::CoreProps(_) => "corePropsPart",
                    PartData::ExtendedProps(_) => "extendedPropsPart",
                    PartData::Comments(_) => "commentsPart",
                    PartData::CommentsExtended(_) => "commentsExtendedPart",
                    PartData::CustomProps(_) => unreachable!(),
                };
                selected[name] = json!(p.path);
            }
            Some(json!({"path":p.path,"kind":kind,"data":data}))
        })
        .collect();
    json!({"parts":parts,"selected":selected})
}
