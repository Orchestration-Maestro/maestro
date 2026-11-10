//! Validate concrete block sequence entries and compose their values.

use super::{
    FrontmatterError,
    composer::{
        Composer,
        collection::{CollectionKind, CollectionPlan},
    },
    cst::{CollectionItem, Node, TokenKind as K},
    resolve_props::PropertyContext,
};

impl Composer<'_> {
    /// Compose sequence members, omitting only trailing comment-only entries.
    pub fn resolve_block_seq<'n>(
        &self,
        node: &Node,
        items: &'n [CollectionItem],
    ) -> Result<CollectionPlan<'n>, FrontmatterError> {
        let mut values = Vec::new();
        for item in items {
            let props = self.resolve_props(
                &item.start,
                PropertyContext {
                    indicator: K::SeqItemInd,
                    flow: false,
                    newline: true,
                    indent: node.indent,
                },
                item.value.as_deref(),
            )?;
            if props.found.is_none()
                && props.anchor.is_none()
                && props.tag.is_none()
                && item.value.is_none()
            {
                continue;
            }
            if props.found.is_none() {
                return Err(self.error_at(node.offset, "Sequence item is missing '-'"));
            }
            if item
                .start
                .iter()
                .any(|token| token.kind == K::SeqItemInd && token.indent != node.indent)
            {
                return Err(
                    self.error_at(node.offset, "Sequence items must start at the same column")
                );
            }
            values.push(self.child(item.value.as_deref(), props)?);
        }
        Ok(CollectionPlan {
            kind: CollectionKind::Sequence,
            children: values,
        })
    }
}
