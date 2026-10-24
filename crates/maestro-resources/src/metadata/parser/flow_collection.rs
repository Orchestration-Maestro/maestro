//! Flow collection item placement and closing delimiter settlement.

use super::{CollectionItem, K, Node, NodeKind, Parser, get_first_key_start_props};

impl Parser {
    /// Place a flow token without interpreting scalar content.
    pub(super) fn flow_collection(&mut self, mut node: Node) -> bool {
        if self.token.kind == K::FlowErrorEnd {
            self.attach(node);
            self.finish_flow_error();
            return false;
        }
        if !node.end.is_empty() {
            return self.flow_end(node);
        }
        let NodeKind::FlowCollection { items, .. } = &mut node.kind else {
            return false;
        };
        let item = items.last_mut();
        match self.token.kind {
            K::Comma | K::ExplicitKeyInd => {
                if let Some(item) = item.filter(|item| item.sep.is_none()) {
                    item.start.push(self.token.clone());
                } else {
                    items.push(CollectionItem {
                        start: vec![self.token.clone()],
                        ..CollectionItem::default()
                    });
                }
            }
            K::MapValueInd => {
                if let Some(item) = item.filter(|item| item.value.is_none()) {
                    item.sep.get_or_insert_default().push(self.token.clone());
                } else {
                    items.push(CollectionItem {
                        sep: Some(vec![self.token.clone()]),
                        ..CollectionItem::default()
                    });
                }
            }
            K::Space | K::Comment | K::Newline | K::Anchor | K::Tag => self.flow_trivia(items),
            kind if kind.is_scalar() => return self.flow_scalar_item(node),
            K::FlowMapEnd | K::FlowSeqEnd => node.end.push(self.token.clone()),
            _ => {
                if let Some(child) = self.start_block_value(&mut node) {
                    self.stack.push(node);
                    self.stack.push(child);
                    return false;
                }
                self.attach(node);
                return true;
            }
        }
        self.stack.push(node);
        false
    }

    /// Finish enclosing flow nodes after a lexer unindent error.
    fn finish_flow_error(&mut self) {
        while matches!(
            self.stack.last().map(|node| &node.kind),
            Some(NodeKind::FlowCollection { .. })
        ) {
            if let Some(node) = self.stack.pop() {
                self.attach(node);
            }
        }
    }

    /// Place flow trivia and properties in the current separator or item start.
    fn flow_trivia(&self, items: &mut Vec<CollectionItem>) {
        if let Some(item) = items.last_mut().filter(|item| item.value.is_none()) {
            item.sep
                .as_mut()
                .unwrap_or(&mut item.start)
                .push(self.token.clone());
        } else {
            items.push(CollectionItem {
                start: vec![self.token.clone()],
                ..CollectionItem::default()
            });
        }
    }

    /// Place a flow scalar as a key or an open value.
    fn flow_scalar_item(&mut self, mut node: Node) -> bool {
        let NodeKind::FlowCollection { items, .. } = &mut node.kind else {
            return false;
        };
        let item = items.last_mut();
        let scalar = Node::new(NodeKind::Scalar(self.token.clone()), &self.token);
        if let Some(item) = item.filter(|item| item.value.is_none()) {
            if item.sep.is_some() {
                self.stack.push(node);
                self.stack.push(scalar);
                return false;
            }
            item.key = Some(Box::new(scalar));
            item.sep = Some(Vec::new());
        } else {
            items.push(CollectionItem {
                key: Some(Box::new(scalar)),
                sep: Some(Vec::new()),
                ..CollectionItem::default()
            });
        }
        self.stack.push(node);
        false
    }

    /// Settle a closed collection or promote it to a block mapping key.
    fn flow_end(&mut self, mut node: Node) -> bool {
        let parent_map = self.stack.last().is_some_and(|parent| matches!(parent.kind, NodeKind::BlockMap(_)) && (self.token.kind == K::MapValueInd && parent.indent == node.indent || self.token.kind == K::Newline && matches!(&parent.kind, NodeKind::BlockMap(items) if items.last().is_some_and(|item| item.sep.is_none()))));
        if parent_map {
            self.attach(node);
            return true;
        }
        if self.token.kind != K::MapValueInd
            || self
                .stack
                .last()
                .is_some_and(|parent| matches!(parent.kind, NodeKind::FlowCollection { .. }))
        {
            return self.line_end(node);
        }
        let start = self
            .previous_props()
            .map(get_first_key_start_props)
            .unwrap_or_default();
        Self::fix_flow_seq_items(&mut node);
        let mut sep = node.end.split_off(1);
        sep.push(self.token.clone());
        let offset = node.offset;
        let indent = node.indent;
        self.stack.push(Node {
            kind: NodeKind::BlockMap(vec![CollectionItem {
                start,
                key: Some(Box::new(node)),
                sep: Some(sep),
                ..CollectionItem::default()
            }]),
            offset,
            indent,
            end: Vec::new(),
        });
        self.on_key_line = true;
        false
    }
}
