//! Link target policy shared by browser renderers.

use crate::browser::docx_preview::{Relationship, WmlHyperlink};

/// Whether a browser would run `target` as script or an active document.
///
/// That is a `javascript:` or `vbscript:` URL, or a `data:` URL whose media type is
/// HTML, XML, SVG or script. Other targets, including unparsable ones, are not refused.
#[must_use]
pub fn executable(target: &str) -> bool {
    let Ok(url) = url::Url::parse(target) else {
        return false;
    };
    match url.scheme() {
        scheme if script_scheme(scheme) => true,
        "data" => {
            let mime = url
                .path()
                .split([',', ';'])
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            matches!(
                mime.as_str(),
                "text/html"
                    | "text/xml"
                    | "application/xml"
                    | "application/xhtml+xml"
                    | "image/svg+xml"
                    | "text/javascript"
                    | "application/javascript"
                    | "application/ecmascript"
                    | "text/ecmascript"
            )
        }
        _ => false,
    }
}

/// Whether a normalized URL scheme is `javascript` or `vbscript`.
#[must_use]
pub fn script_scheme(scheme: &str) -> bool {
    matches!(scheme, "javascript" | "vbscript")
}

/// The `href` a rendered Word hyperlink carries, or `None` when its target is refused.
///
/// The target is the first relationship whose identifier matches and whose target mode is
/// `External`, followed by `#` and the anchor when one is declared.
#[must_use]
pub fn hyperlink_href(
    link: &WmlHyperlink,
    relationships: Option<&[Relationship]>,
) -> Option<String> {
    let target = external_target(link, relationships).unwrap_or_default();
    let href = link
        .anchor
        .as_deref()
        .filter(|anchor| !anchor.is_empty())
        .map_or_else(|| target.to_owned(), |anchor| format!("{target}#{anchor}"));
    (!executable(&href)).then_some(href)
}

/// The first external target declared for `link`'s identifier.
fn external_target<'a>(
    link: &WmlHyperlink,
    relationships: Option<&'a [Relationship]>,
) -> Option<&'a str> {
    let id = link.id.as_deref().filter(|id| !id.is_empty())?;
    let rel = relationships?.iter().find(|rel| {
        rel.id.as_deref() == Some(id) && rel.target_mode.as_deref() == Some("External")
    })?;
    rel.target.as_deref()
}
