//! Complete ordered observations of prepared Word part records.
#![cfg(test)]
use maestro_app::browser::docx_preview::{IDomStyle, OpenXmlElementData as Data};
use serde_json::{Value, json};

/// Add an encountered field, keeping null distinct from absence.
fn member<T>(out: &mut Value, name: &str, value: Option<T>, encode: impl FnOnce(T) -> Value) {
    if let Some(value) = value {
        out[name] = encode(value);
    }
}
/// Observe complete style records with ordered CSS declarations.
pub fn styles(styles: Vec<IDomStyle>) -> Value {
    json!(styles.into_iter().map(style).collect::<Vec<_>>())
}
/// Observe one style's CSS and independent raw metadata.
fn style(style: IDomStyle) -> Value {
    let mut out = json!({
        "id":style.id, "name":style.name, "target":style.target, "basedOn":style.based_on,
        "styles":style.styles.into_iter().map(|s| {
            let mut v = json!({"target":s.target,"values":s.values.into_iter().collect::<Vec<_>>()});
            member(&mut v, "mod", s.r#mod, |v| json!(v));
            v
        }).collect::<Vec<_>>()
    });
    member(&mut out, "isDefault", style.is_default, |v| json!(v));
    member(&mut out, "linked", style.linked, |v| json!(v));
    member(&mut out, "next", style.next, |v| json!(v));
    member(&mut out, "aliases", style.aliases, |v| json!(v));
    member(&mut out, "paragraphProps", style.paragraph_props, |p| {
        let mut v = json!({});
        super::body_tree::formatting(
            &Data::Paragraph {
                properties: Box::new(p),
            },
            &mut v,
        );
        v
    });
    member(&mut out, "runProps", style.run_props, |p| {
        let mut v = json!({});
        super::body_tree::common(&p, &mut v);
        v
    });
    out
}

/// Observe both optional schemes and ordered color entries.
pub fn theme(theme: maestro_app::browser::docx_preview::DmlTheme) -> Value {
    let mut out = json!({});
    member(
        &mut out,
        "colorScheme",
        theme.color_scheme,
        |s| json!({"name":s.name,"colors":s.colors.into_iter().collect::<Vec<_>>()}),
    );
    member(&mut out, "fontScheme", theme.font_scheme, |s| {
        let mut v = json!({"name":s.name});
        member(&mut v, "majorFont", s.major_font, font);
        member(&mut v, "minorFont", s.minor_font, font);
        v
    });
    out
}
/// Observe encountered typefaces without flattening nullable attributes.
fn font(font: maestro_app::browser::docx_preview::DmlFormInfo) -> Value {
    let typeface = |value: Option<Option<String>>| {
        value.map_or_else(|| json!({"$undefined":true}), |v| json!(v))
    };
    json!({
        "latinTypeface":typeface(font.latin_typeface),
        "eaTypeface":typeface(font.ea_typeface),
        "csTypeface":typeface(font.cs_typeface)
    })
}

/// Observe the typed header root and complete arena after input release.
pub fn header(header: maestro_app::browser::docx_preview::WmlHeader) -> Value {
    json!({"type":"header","cssStyle":header.css_style.into_iter().collect::<Vec<_>>(),
        "children":super::body_tree::full_graph(&header.children)})
}

/// Observe the typed footer root and complete arena after input release.
pub fn footer(footer: maestro_app::browser::docx_preview::WmlFooter) -> Value {
    json!({"type":"footer","cssStyle":footer.css_style.into_iter().collect::<Vec<_>>(),
        "children":super::body_tree::full_graph(&footer.children)})
}

/// Observe shared note data and the caller's statically selected kind.
pub fn note(note: &maestro_app::browser::docx_preview::WmlBaseNote, kind: &str) -> Value {
    json!({"type":kind,"id":note.id,"noteType":note.note_type,
        "children":super::body_tree::full_graph(&note.children)})
}

/// Observe encountered settings and independent note-default groups.
pub fn settings(settings: maestro_app::browser::docx_preview::WmlSettings) -> Value {
    let mut out = json!({});
    member(&mut out, "defaultTabStop", settings.default_tab_stop, |v| {
        json!(v)
    });
    member(
        &mut out,
        "autoHyphenation",
        settings.auto_hyphenation,
        |v| json!(v),
    );
    member(
        &mut out,
        "footnoteProps",
        settings.footnote_props,
        note_properties,
    );
    member(
        &mut out,
        "endnoteProps",
        settings.endnote_props,
        note_properties,
    );
    out
}
/// Observe nullable default IDs in source order.
fn note_properties(props: maestro_app::browser::docx_preview::NoteProperties) -> Value {
    let mut out = json!({"defaultNoteIds":props.default_note_ids});
    member(&mut out, "nummeringFormat", props.nummering_format, |v| {
        json!(v)
    });
    out
}
