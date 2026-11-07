//! Embedded font faces.
use super::{check_preview, markup, package};
use crate::docx_preview::{Options, parse_async};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde_json::Value;

#[test]
fn embedded_fonts_become_font_faces_with_decoded_bytes() {
    check_preview("embedded-fonts");
    check_preview("embedded-fonts-data-urls");
}

#[test]
fn ignored_fonts_emit_no_font_faces() {
    check_preview("embedded-fonts-ignored");
}

#[test]
fn font_names_stay_inside_the_font_face_rule() {
    check_preview("font-name-escape");
}

/// A real obfuscated face decodes back to the font file it was embedded from.
#[test]
fn an_obfuscated_face_decodes_to_its_original_font() {
    let mut options = Options::default();
    options.resources.use_base64_url = true;
    let rendering = parse_async(&package("embedded-font-loaded"), options.clone())
        .unwrap()
        .render(&options);
    let face = markup(&rendering.resource_styles[0]);
    let encoded = face
        .split("base64,")
        .nth(1)
        .unwrap()
        .split(')')
        .next()
        .unwrap();
    assert_eq!(
        STANDARD.decode(encoded).unwrap(),
        include_bytes!("../../../assets/katex/fonts/KaTeX_Typewriter-Regular.woff2")
    );
}

/// Every package the browser artifact tests read is a committed fixture package, byte for byte.
#[test]
fn artifact_packages_match_their_committed_fixtures() {
    let packages: Vec<Value> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/native_document_artifacts.json"
    ))
    .unwrap();
    for entry in packages {
        let name = entry["name"].as_str().unwrap();
        let content = STANDARD.decode(entry["content"].as_str().unwrap()).unwrap();
        assert!(
            content == package(name),
            "{name} matches its fixture package"
        );
    }
}
