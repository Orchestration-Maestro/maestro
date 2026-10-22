//! Extended comment links and shared lookup records.
use super::super::{XmlElement, XmlParser, utils::key_by};
use crate::utils::lock::read;
use indexmap::IndexMap;
use std::sync::{Arc, RwLock};
/// Raw paragraph links and an optional completion token.
#[derive(Debug, Default)]
pub struct CommentsExtended {
    /// Nullable paragraph identifier.
    pub para_id: Option<String>,
    /// Nullable parent paragraph identifier.
    pub para_id_parent: Option<String>,
    /// Recognized completion flag.
    pub done: Option<bool>,
}
/// Prepared links with source-order records and snapshot lookup keys.
#[derive(Debug, Default)]
pub struct CommentsExtendedPart {
    /// Source-order shared records, including duplicates.
    pub comments: Vec<Arc<RwLock<CommentsExtended>>>,
    /// Lookup indexed by snapshot identifiers.
    pub comment_map: IndexMap<String, Arc<RwLock<CommentsExtended>>>,
}
impl CommentsExtendedPart {
    /// Prepare links and last-key-wins lookups sharing the source list records.
    #[must_use]
    pub fn parse_xml<'doc, 'input: 'doc>(
        xml: &'doc XmlParser<'input>,
        root: XmlElement<'doc, 'input>,
    ) -> Self {
        let comments: Vec<_> = xml
            .elements(root, Some("commentEx"))
            .map(|c| {
                Arc::new(RwLock::new(CommentsExtended {
                    para_id: xml.attr(c, "paraId").map(str::to_owned),
                    para_id_parent: xml.attr(c, "paraIdParent").map(str::to_owned),
                    done: xml.bool_attr(c, "done", None),
                }))
            })
            .collect();
        let comment_map = key_by(comments.iter().map(|comment| {
            let key = read(comment).para_id.clone();
            (key, Arc::clone(comment))
        }));
        Self {
            comments,
            comment_map,
        }
    }
}
