//! Immutable language descriptors.
use super::{Language, ModeId};

/// Derived from `highlight.js/lib/languages/xml.js`, version 11.9.0.
/// SHA-256: dcb9cd430937eb3b1416b385d9fcda72fa9233dcfe93bf0146235f6c3a2b6487.
pub(super) const VIEWER: Language = Language {
    name: "xml",
    root: Some(ModeId(445)),
    aliases: &[
        "html", "xhtml", "rss", "atom", "xjb", "xsd", "xsl", "plist", "wsf", "svg",
    ],
    superset: None,
    fold: true,
};

/// Derived from `highlight.js/lib/languages/xml.js`, version 11.11.1.
/// SHA-256: dcb9cd430937eb3b1416b385d9fcda72fa9233dcfe93bf0146235f6c3a2b6487.
pub(super) const BROWSER_HTML: Language = Language {
    name: "html",
    root: Some(ModeId(445)),
    aliases: &[
        "html", "xhtml", "rss", "atom", "xjb", "xsd", "xsl", "plist", "wsf", "svg",
    ],
    superset: None,
    fold: true,
};

/// Derived from `highlight.js/lib/languages/xml.js`, version 11.11.1.
/// SHA-256: dcb9cd430937eb3b1416b385d9fcda72fa9233dcfe93bf0146235f6c3a2b6487.
pub(super) const BROWSER: Language = Language {
    name: "xml",
    root: Some(ModeId(445)),
    aliases: &[
        "html", "xhtml", "rss", "atom", "xjb", "xsd", "xsl", "plist", "wsf", "svg",
    ],
    superset: None,
    fold: true,
};
