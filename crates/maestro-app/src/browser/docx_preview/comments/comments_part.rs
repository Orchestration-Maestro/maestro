//! Comment part preparation and shared lookup records.
use super::super::{
    DocxError, OwnedXmlElement, document_parser::comments::parse_comments, utils::key_by,
};
use super::WmlComment;
use crate::utils::lock::read;
use indexmap::IndexMap;
use std::sync::{Arc, RwLock};
/// Prepared comments with source-order records and snapshot lookup keys.
#[derive(Debug, Default)]
pub struct CommentsPart {
    /// Source-order shared records, including duplicates.
    pub comments: Vec<Arc<RwLock<WmlComment>>>,
    /// Lookup indexed by snapshot identifiers.
    pub comment_map: IndexMap<String, Arc<RwLock<WmlComment>>>,
}
impl CommentsPart {
    /// Prepare comments and last-key-wins lookups sharing the source list records.
    /// `ignore_width` suppresses cell widths in comment bodies.
    ///
    /// # Errors
    /// Propagates XML admission, selection or body failures.
    pub fn parse_xml(input: &OwnedXmlElement, ignore_width: bool) -> Result<Self, DocxError> {
        let comments: Vec<_> = parse_comments(input, ignore_width)?
            .into_iter()
            .map(|c| Arc::new(RwLock::new(c)))
            .collect();
        let comment_map = key_by(comments.iter().map(|comment| {
            let key = read(comment).id.clone();
            (key, Arc::clone(comment))
        }));
        Ok(Self {
            comments,
            comment_map,
        })
    }
}
