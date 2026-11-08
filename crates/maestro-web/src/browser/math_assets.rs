//! Matching math stylesheet and fonts, embedded and installed once per document.
use crate::browser::attachment_content::{AttachmentHostError, browser::host_error};
use std::hash::{DefaultHasher, Hash as _, Hasher as _};
use web_sys::{Document, FontFace, FontFaceDescriptors};

/// Opening of each font rule in the stylesheet.
const FONT_RULE: &str = "@font-face{";
/// Stylesheet whose font sources are the embedded files below.
const STYLESHEET: &str = include_str!("../../assets/katex/katex.css");
/// Embedded fonts, named as the stylesheet refers to them.
const FONTS: [(&str, &[u8]); 20] = [
    (
        "KaTeX_AMS-Regular",
        include_bytes!("../../assets/katex/fonts/KaTeX_AMS-Regular.woff2"),
    ),
    (
        "KaTeX_Caligraphic-Bold",
        include_bytes!("../../assets/katex/fonts/KaTeX_Caligraphic-Bold.woff2"),
    ),
    (
        "KaTeX_Caligraphic-Regular",
        include_bytes!("../../assets/katex/fonts/KaTeX_Caligraphic-Regular.woff2"),
    ),
    (
        "KaTeX_Fraktur-Bold",
        include_bytes!("../../assets/katex/fonts/KaTeX_Fraktur-Bold.woff2"),
    ),
    (
        "KaTeX_Fraktur-Regular",
        include_bytes!("../../assets/katex/fonts/KaTeX_Fraktur-Regular.woff2"),
    ),
    (
        "KaTeX_Main-Bold",
        include_bytes!("../../assets/katex/fonts/KaTeX_Main-Bold.woff2"),
    ),
    (
        "KaTeX_Main-BoldItalic",
        include_bytes!("../../assets/katex/fonts/KaTeX_Main-BoldItalic.woff2"),
    ),
    (
        "KaTeX_Main-Italic",
        include_bytes!("../../assets/katex/fonts/KaTeX_Main-Italic.woff2"),
    ),
    (
        "KaTeX_Main-Regular",
        include_bytes!("../../assets/katex/fonts/KaTeX_Main-Regular.woff2"),
    ),
    (
        "KaTeX_Math-BoldItalic",
        include_bytes!("../../assets/katex/fonts/KaTeX_Math-BoldItalic.woff2"),
    ),
    (
        "KaTeX_Math-Italic",
        include_bytes!("../../assets/katex/fonts/KaTeX_Math-Italic.woff2"),
    ),
    (
        "KaTeX_SansSerif-Bold",
        include_bytes!("../../assets/katex/fonts/KaTeX_SansSerif-Bold.woff2"),
    ),
    (
        "KaTeX_SansSerif-Italic",
        include_bytes!("../../assets/katex/fonts/KaTeX_SansSerif-Italic.woff2"),
    ),
    (
        "KaTeX_SansSerif-Regular",
        include_bytes!("../../assets/katex/fonts/KaTeX_SansSerif-Regular.woff2"),
    ),
    (
        "KaTeX_Script-Regular",
        include_bytes!("../../assets/katex/fonts/KaTeX_Script-Regular.woff2"),
    ),
    (
        "KaTeX_Size1-Regular",
        include_bytes!("../../assets/katex/fonts/KaTeX_Size1-Regular.woff2"),
    ),
    (
        "KaTeX_Size2-Regular",
        include_bytes!("../../assets/katex/fonts/KaTeX_Size2-Regular.woff2"),
    ),
    (
        "KaTeX_Size3-Regular",
        include_bytes!("../../assets/katex/fonts/KaTeX_Size3-Regular.woff2"),
    ),
    (
        "KaTeX_Size4-Regular",
        include_bytes!("../../assets/katex/fonts/KaTeX_Size4-Regular.woff2"),
    ),
    (
        "KaTeX_Typewriter-Regular",
        include_bytes!("../../assets/katex/fonts/KaTeX_Typewriter-Regular.woff2"),
    ),
];

/// Add the embedded fonts to the document's font set and insert the stylesheet without its
/// font rules, once per document and content hash.
///
/// The faces are built from their bytes, which no page content policy governs, so math keeps
/// its fonts on a page that refuses font URLs.
pub(crate) fn install(document: &Document) -> Result<(), AttachmentHostError> {
    let mut hasher = DefaultHasher::new();
    STYLESHEET.hash(&mut hasher);
    for font in FONTS {
        font.hash(&mut hasher);
    }
    let id = format!("maestro-math-{:016x}", hasher.finish());
    if document.get_element_by_id(&id).is_some() {
        return Ok(());
    }
    let mut css = String::with_capacity(STYLESHEET.len());
    let mut rest = STYLESHEET;
    while let Some(start) = rest.find(FONT_RULE) {
        css.push_str(&rest[..start]);
        let rule = &rest[start + FONT_RULE.len()..];
        let end = rule
            .find('}')
            .ok_or_else(|| AttachmentHostError::Presentation("Unterminated font rule".into()))?;
        add_face(document, &rule[..end])?;
        rest = &rule[end + 1..];
    }
    css.push_str(rest);
    super::install_stylesheet(document, &id, &css)
}

/// Add the embedded face whose font rule has the declarations `rule`.
fn add_face(document: &Document, rule: &str) -> Result<(), AttachmentHostError> {
    let descriptors = FontFaceDescriptors::new();
    let (mut family, mut file) = ("", "");
    for (name, value) in rule.split(';').filter_map(|line| line.split_once(':')) {
        match name {
            "font-family" => family = value.trim_matches('"'),
            "font-style" => descriptors.set_style(value),
            "font-weight" => descriptors.set_weight(value),
            "font-display" => descriptors.set_display(value),
            "src" => {
                file = value
                    .strip_prefix("url(fonts/")
                    .and_then(|source| source.split_once(".woff2)"))
                    .map_or("", |(file, _)| file);
            }
            _ => {}
        }
    }
    let (_, bytes) = FONTS
        .iter()
        .find(|(name, _)| *name == file)
        .ok_or_else(|| AttachmentHostError::Presentation(format!("Missing math font {file}")))?;
    let face = FontFace::new_with_u8_array_and_descriptors(family, bytes, &descriptors)
        .map_err(|error| host_error(&error))?;
    document
        .fonts()
        .add(&face)
        .map_err(|error| host_error(&error))
}
