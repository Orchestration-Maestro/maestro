//! Validate concrete block mapping entries and prepare their pairs.

use super::{
    FrontmatterError,
    composer::{
        Composer,
        collection::{CollectionKind, CollectionPlan},
    },
    cst::{CollectionItem, Node, NodeKind, TokenKind as K},
    resolve_props::{Properties, PropertyContext},
};

impl Composer<'_> {
    /// Prepare mapping children in authored key/value order.
    pub fn resolve_block_map<'n>(
        &self,
        node: &Node,
        items: &'n [CollectionItem],
    ) -> Result<CollectionPlan<'n>, FrontmatterError> {
        let mut children = Vec::new();
        for item in items {
            let props = self.resolve_props(
                &item.start,
                PropertyContext {
                    indicator: K::ExplicitKeyInd,
                    flow: false,
                    newline: true,
                    indent: node.indent,
                },
                item.key.as_deref(),
            )?;
            if props.found.is_none()
                && props.anchor.is_none()
                && props.tag.is_none()
                && item.sep.is_none()
            {
                continue;
            }
            self.check_block_key(node, item, &props)?;
            let value_props = self.resolve_props(
                item.sep.as_deref().unwrap_or_default(),
                PropertyContext {
                    indicator: K::MapValueInd,
                    flow: false,
                    newline: item.key.is_none(),
                    indent: node.indent,
                },
                item.value.as_deref(),
            )?;
            self.check_block_value(node, item, &props, &value_props)?;
            children.push(self.child(item.key.as_deref(), props)?);
            children.push(self.child(item.value.as_deref(), value_props)?);
        }
        Ok(CollectionPlan {
            kind: CollectionKind::Mapping,
            children,
        })
    }

    /// Validate implicit key extent and mapping indentation.
    fn check_block_key(
        &self,
        node: &Node,
        item: &CollectionItem,
        props: &Properties,
    ) -> Result<(), FrontmatterError> {
        if props.found.is_none()
            && (props.newline_after_prop
                || item
                    .key
                    .as_deref()
                    .is_some_and(|key| self.contains_newline(key)))
        {
            return Err(self.error_at(node.offset, "Implicit keys must be on one line"));
        }
        let bad_implicit = props.found.is_none()
            && item.key.as_deref().is_some_and(|key| {
                key.indent != node.indent || matches!(key.kind, NodeKind::BlockSequence(_))
            });
        let bad_explicit = props
            .found
            .as_ref()
            .is_some_and(|token| token.indent != node.indent);
        if bad_implicit || bad_explicit {
            return Err(self.error_at(node.offset, "Mapping items must start at the same column"));
        }
        Ok(())
    }

    /// Validate a colon and the compact implicit-key value restrictions.
    fn check_block_value(
        &self,
        node: &Node,
        item: &CollectionItem,
        props: &Properties,
        value_props: &Properties,
    ) -> Result<(), FrontmatterError> {
        if props.found.is_some() {
            return Ok(());
        }
        let Some(colon) = &value_props.found else {
            return Err(self.error_at(node.offset, "Expected ':' after mapping key"));
        };
        if item
            .value
            .as_ref()
            .is_some_and(|value| matches!(value.kind, NodeKind::BlockMap(_)))
            && !value_props.newline
        {
            return Err(self.error_at(
                node.offset,
                "Nested mappings are not allowed in compact mappings",
            ));
        }
        if self.implicit_key_too_long(item.key.as_deref(), props, colon) {
            return Err(self.error_at(colon.offset, "Implicit mapping key exceeds 1024 characters"));
        }
        Ok(())
    }
}
