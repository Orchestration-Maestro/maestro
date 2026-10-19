//! Link target policy shared by browser renderers.

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
        "javascript" | "vbscript" => true,
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
