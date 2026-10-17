//! Matching math stylesheet and fonts, embedded and installed once per document.
use crate::browser::attachment_content::AttachmentUiError;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::hash::{DefaultHasher, Hash as _, Hasher as _};
use web_sys::Document;

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

/// Insert the stylesheet with its fonts as data, once per document and content hash.
pub(crate) fn install(document: &Document) -> Result<(), AttachmentUiError> {
    let mut hasher = DefaultHasher::new();
    STYLESHEET.hash(&mut hasher);
    for font in FONTS {
        font.hash(&mut hasher);
    }
    let css = FONTS
        .iter()
        .fold(STYLESHEET.to_owned(), |css, (name, bytes)| {
            css.replace(
                &format!("url(fonts/{name}.woff2)"),
                &format!("url(data:font/woff2;base64,{})", STANDARD.encode(bytes)),
            )
        });
    super::install_stylesheet(
        document,
        &format!("maestro-math-{:016x}", hasher.finish()),
        &css,
    )
}
