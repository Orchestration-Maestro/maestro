//! Validate concrete flow collections and compose sequence pairs.

use super::{
    FrontmatterError,
    composer::{
        Composer,
        collection::{Child, CollectionKind, CollectionPlan},
    },
    cst::{CollectionItem, Node, NodeKind, SourceToken, TokenKind as K},
    resolve_props::PropertyContext,
};

/// Delimiters and member construction for a flow collection.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Collection {
    /// Square-bracketed sequence.
    Sequence,
    /// Brace-delimited mapping.
    Mapping,
}

impl Collection {
    /// Required closing delimiter.
    fn close(self) -> K {
        match self {
            Self::Sequence => K::FlowSeqEnd,
            Self::Mapping => K::FlowMapEnd,
        }
    }
    /// Existing error template for an unfinished collection.
    fn error(self) -> &'static str {
        match self {
            Self::Sequence => "while parsing a flow sequence, expected ',' or ']'",
            Self::Mapping => "while parsing a flow mapping, expected ',' or '}'",
        }
    }
}

impl Composer<'_> {
    /// Compose flow entries, requiring commas and the matching closer.
    pub fn resolve_flow_collection<'n>(
        &self,
        node: &Node,
        start: &SourceToken,
        items: &'n [CollectionItem],
    ) -> Result<CollectionPlan<'n>, FrontmatterError> {
        let kind = if start.kind == K::FlowMapStart {
            Collection::Mapping
        } else {
            Collection::Sequence
        };
        let mut children = Vec::new();
        let mut pairs = Vec::new();
        let mut offset = start.end;
        for (index, item) in items.iter().enumerate() {
            let props = self.resolve_props(
                &item.start,
                PropertyContext {
                    indicator: K::ExplicitKeyInd,
                    flow: true,
                    newline: false,
                    indent: node.indent,
                },
                item.key.as_deref().or(item.value.as_deref()),
            )?;
            let empty = props.found.is_none()
                && props.anchor.is_none()
                && props.tag.is_none()
                && item.sep.is_none()
                && item.value.is_none();
            if empty && (index == 0 && props.comma.is_some() || index + 1 < items.len()) {
                return Err(self.error_at(offset, kind.error()));
            }
            if empty {
                offset = offset.max(props.end);
                continue;
            }
            if index == 0 && props.comma.is_some() || index > 0 && props.comma.is_none() {
                return Err(self.error_at(offset, kind.error()));
            }
            if kind == Collection::Sequence && item.sep.is_none() && props.found.is_none() {
                self.reject_block(item.value.as_deref())?;
                children.push(self.child(item.value.as_deref(), props)?);
                pairs.push(false);
            } else {
                children.extend(self.flow_pair(node, item, props, kind)?);
                pairs.push(true);
            }
            offset = offset.max(Self::item_end(item));
        }
        let Some(closer) = node.end.first().filter(|token| token.kind == kind.close()) else {
            return Err(self.error_at(offset, kind.error()));
        };
        self.resolve_end(&node.end[1..], false)?;
        if closer.offset < offset {
            return Err(self.error_at(offset, kind.error()));
        }
        Ok(CollectionPlan {
            kind: if kind == Collection::Mapping {
                CollectionKind::Mapping
            } else {
                CollectionKind::FlowSequence(pairs)
            },
            children,
        })
    }

    /// Compose one flow pair after validating its separator list.
    fn flow_pair<'n>(
        &self,
        node: &Node,
        item: &'n CollectionItem,
        props: super::resolve_props::Properties,
        kind: Collection,
    ) -> Result<[Child<'n>; 2], FrontmatterError> {
        self.reject_block(item.key.as_deref())?;
        if kind == Collection::Sequence
            && props.found.is_none()
            && item
                .key
                .as_deref()
                .is_some_and(|key| self.contains_newline(key))
        {
            return Err(self.error_at(node.offset, "Implicit flow pair key must be on one line"));
        }
        let sep = item.sep.as_deref().unwrap_or_default();
        let value_props = self.resolve_props(
            sep,
            PropertyContext {
                indicator: K::MapValueInd,
                flow: true,
                newline: false,
                indent: node.indent,
            },
            item.value.as_deref(),
        )?;
        if value_props.found.is_none() && item.value.is_some() {
            return Err(self.error_at(value_props.end, kind.error()));
        }
        if kind == Collection::Sequence && props.found.is_none() && value_props.found.is_some() {
            let colon = sep.iter().find(|token| token.kind == K::MapValueInd);
            if let Some(colon) = colon
                && (self.implicit_key_too_long(item.key.as_deref(), &props, colon)
                    || sep
                        .iter()
                        .take_while(|token| token.offset < colon.offset)
                        .any(|token| token.kind == K::Newline))
            {
                return Err(self.error_at(colon.offset, "Invalid implicit flow pair key"));
            }
        }
        self.reject_block(item.value.as_deref())?;
        Ok([
            self.child(item.key.as_deref(), props)?,
            self.child(item.value.as_deref(), value_props)?,
        ])
    }

    /// Reject block-style children in flow context.
    fn reject_block(&self, node: Option<&Node>) -> Result<(), FrontmatterError> {
        if let Some(node) = node.filter(|node| {
            matches!(
                node.kind,
                NodeKind::BlockMap(_) | NodeKind::BlockSequence(_)
            )
        }) {
            return Err(self.error_at(
                node.offset,
                "Block collection is not allowed in flow collection",
            ));
        }
        Ok(())
    }

    /// End position retained by an entry's last node or property token.
    fn item_end(item: &CollectionItem) -> usize {
        let props = item
            .sep
            .as_ref()
            .and_then(|sep| sep.last())
            .or(item.start.last())
            .map_or(0, |token| token.end);
        props.max(
            item.value
                .as_deref()
                .or(item.key.as_deref())
                .map_or(0, Self::node_end),
        )
    }

    /// Find a node's final token without scanning the document source.
    fn node_end(mut node: &Node) -> usize {
        loop {
            if let Some(last) = node.end.last() {
                return last.end;
            }
            node = match Self::node_successor(node) {
                Ok(child) => child,
                Err(end) => return end,
            };
        }
    }

    /// Follow the final child or return a leaf's retained end position.
    fn node_successor(node: &Node) -> Result<&Node, usize> {
        match &node.kind {
            NodeKind::Scalar(token) => Err(token.end),
            NodeKind::BlockScalar { source, props } => Err(source
                .as_ref()
                .or(props.last())
                .map_or(node.offset, |token| token.end)),
            NodeKind::BlockMap(items)
            | NodeKind::BlockSequence(items)
            | NodeKind::FlowCollection { items, .. } => {
                let Some(item) = items.last() else {
                    return Err(node.offset + 1);
                };
                item.value
                    .as_deref()
                    .or(item.key.as_deref())
                    .ok_or_else(|| Self::item_end(item))
            }
            _ => Err(node.offset),
        }
    }
}
