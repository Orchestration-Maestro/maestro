//! Owned header and footer roots.
use super::super::OpenXmlElementTree;
/// Header root with its owned body tree.
#[derive(Debug, Default)]
pub struct WmlHeader {
    /// Parsed immediate body content.
    pub children: OpenXmlElementTree,
    /// Root CSS values, initially empty when parsed.
    pub css_style: indexmap::IndexMap<String, Option<String>>,
}

/// Footer root with its owned body tree.
#[derive(Debug, Default)]
pub struct WmlFooter {
    /// Parsed immediate body content.
    pub children: OpenXmlElementTree,
    /// Root CSS values, initially empty when parsed.
    pub css_style: indexmap::IndexMap<String, Option<String>>,
}
