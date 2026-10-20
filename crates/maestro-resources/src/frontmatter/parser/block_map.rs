//! Block mapping item placement on the iterative parser stack.

use super::{CollectionItem, K, Node, NodeKind, Parser, get_first_key_start_props, includes_token};

impl Parser {
    /// Place the current token in a block mapping or settle an unindented map.
    pub(super) fn block_map(&mut self, mut node: Node) -> bool {
        if self.token.kind.is_trivia() {
            if self.token.kind == K::Newline {
                self.on_key_line = false;
            }
            self.collection_trivia(&mut node);
            self.stack.push(node);
            return false;
        }
        if self.token.indent < node.indent {
            self.attach(node);
            return true;
        }
        let at_indent = !self.on_key_line && self.token.indent == node.indent;
        let NodeKind::BlockMap(items) = &mut node.kind else {
            return false;
        };
        let Some(item) = items.last_mut() else {
            return false;
        };
        let next = at_indent
            && (item.sep.is_some() || item.explicit_key)
            && self.token.kind != K::SeqItemInd;
        let start = Self::split_empty_props(item, next, node.indent);
        match self.token.kind {
            K::Anchor | K::Tag => {
                if next || item.value.is_some() {
                    let mut start = start;
                    start.push(self.token.clone());
                    items.push(CollectionItem {
                        start,
                        ..CollectionItem::default()
                    });
                    self.on_key_line = true;
                } else if let Some(sep) = &mut item.sep {
                    sep.push(self.token.clone());
                } else {
                    item.start.push(self.token.clone());
                }
                self.stack.push(node);
                false
            }
            K::ExplicitKeyInd => {
                self.explicit_key(node, start, next);
                false
            }
            K::MapValueInd => {
                self.map_value(node, start, next);
                false
            }
            kind if kind.is_scalar() => self.mapping_scalar(node, start, next),
            _ => self.map_child(node, start, at_indent),
        }
    }

    /// Place a scalar as the current key or an open value.
    fn mapping_scalar(
        &mut self,
        mut node: Node,
        start: Vec<super::SourceToken>,
        next: bool,
    ) -> bool {
        let NodeKind::BlockMap(items) = &mut node.kind else {
            return false;
        };
        let Some(item) = items.last_mut() else {
            return false;
        };
        let scalar = Node::new(NodeKind::Scalar(self.token.clone()), &self.token);
        if next || item.value.is_some() {
            items.push(CollectionItem {
                start,
                key: Some(Box::new(scalar)),
                sep: Some(Vec::new()),
                ..CollectionItem::default()
            });
            self.on_key_line = true;
            self.stack.push(node);
        } else if item.sep.is_some() {
            self.stack.push(node);
            self.stack.push(scalar);
        } else {
            item.key = Some(Box::new(scalar));
            item.sep = Some(Vec::new());
            self.on_key_line = true;
            self.stack.push(node);
        }
        false
    }

    /// Move newline-separated properties after an empty value to the next item.
    fn split_empty_props(
        item: &mut CollectionItem,
        next: bool,
        indent: usize,
    ) -> Vec<super::SourceToken> {
        if !next || item.value.is_some() {
            return Vec::new();
        }
        let Some(sep) = &mut item.sep else {
            return Vec::new();
        };
        let mut newlines = Vec::new();
        for (index, token) in sep.iter().enumerate() {
            match token.kind {
                K::Newline => newlines.push(index),
                K::Space => (),
                K::Comment if token.indent <= indent => (),
                _ => newlines.clear(),
            }
        }
        if newlines.len() >= 2 {
            sep.split_off(newlines[1])
        } else {
            Vec::new()
        }
    }

    /// Place an explicit key indicator in its owning entry or nested mapping.
    fn explicit_key(&mut self, mut node: Node, mut start: Vec<super::SourceToken>, next: bool) {
        let NodeKind::BlockMap(items) = &mut node.kind else {
            return;
        };
        let Some(item) = items.last_mut() else {
            return;
        };
        let child = if item.sep.is_none() && !item.explicit_key {
            item.start.push(self.token.clone());
            item.explicit_key = true;
            None
        } else if next || item.value.is_some() {
            start.push(self.token.clone());
            items.push(CollectionItem {
                start,
                explicit_key: true,
                ..CollectionItem::default()
            });
            None
        } else {
            Some(Node::new(
                NodeKind::BlockMap(vec![CollectionItem {
                    start: vec![self.token.clone()],
                    explicit_key: true,
                    ..CollectionItem::default()
                }]),
                &self.token,
            ))
        };
        self.stack.push(node);
        if let Some(child) = child {
            self.stack.push(child);
        }
        self.on_key_line = true;
    }

    /// Place a colon using the existing entry's separator presence.
    fn map_value(&mut self, mut node: Node, start: Vec<super::SourceToken>, next: bool) {
        let NodeKind::BlockMap(items) = &mut node.kind else {
            return;
        };
        let Some(item) = items.last_mut() else {
            return;
        };
        let child = if item.explicit_key {
            self.explicit_value(items, start)
        } else if item.sep.is_none() {
            item.sep = Some(vec![self.token.clone()]);
            None
        } else if item.value.is_some() || next {
            items.push(CollectionItem {
                start,
                sep: Some(vec![self.token.clone()]),
                ..CollectionItem::default()
            });
            None
        } else if item
            .sep
            .as_ref()
            .is_some_and(|sep| includes_token(sep, K::MapValueInd))
        {
            Some(self.empty_map(Vec::new()))
        } else {
            if let Some(sep) = &mut item.sep {
                sep.push(self.token.clone());
            }
            None
        };
        self.stack.push(node);
        if let Some(child) = child {
            self.stack.push(child);
        }
        self.on_key_line = true;
    }

    /// Resolve colon placement after an explicit key.
    fn explicit_value(
        &self,
        items: &mut Vec<CollectionItem>,
        start: Vec<super::SourceToken>,
    ) -> Option<Node> {
        let item = items.last_mut()?;
        if item.sep.is_none() {
            if includes_token(&item.start, K::Newline) {
                item.sep = Some(vec![self.token.clone()]);
                return None;
            }
            return Some(self.empty_map(get_first_key_start_props(&mut item.start)));
        }
        if item.value.is_some() {
            items.push(CollectionItem {
                sep: Some(vec![self.token.clone()]),
                ..CollectionItem::default()
            });
            return None;
        }
        let sep = item.sep.as_mut()?;
        if includes_token(sep, K::MapValueInd) {
            return Some(self.empty_map(start));
        }
        if item.key.as_ref().is_some_and(|key| key.is_flow()) && !includes_token(sep, K::Newline) {
            let start = get_first_key_start_props(&mut item.start);
            let mut sep = item.sep.take().unwrap_or_default();
            sep.push(self.token.clone());
            let key = item.key.take();
            return Some(Node::new(
                NodeKind::BlockMap(vec![CollectionItem {
                    start,
                    key,
                    sep: Some(sep),
                    ..CollectionItem::default()
                }]),
                &self.token,
            ));
        }
        sep.extend(start);
        sep.push(self.token.clone());
        None
    }

    /// Construct a mapping whose first key is absent.
    fn empty_map(&self, start: Vec<super::SourceToken>) -> Node {
        Node::new(
            NodeKind::BlockMap(vec![CollectionItem {
                start,
                sep: Some(vec![self.token.clone()]),
                ..CollectionItem::default()
            }]),
            &self.token,
        )
    }

    /// Start a non-scalar child, rejecting an inline implicit-key sequence.
    fn map_child(
        &mut self,
        mut node: Node,
        start: Vec<super::SourceToken>,
        at_indent: bool,
    ) -> bool {
        let Some(child) = self.start_block_value(&mut node) else {
            self.attach(node);
            return true;
        };
        let NodeKind::BlockMap(items) = &mut node.kind else {
            return false;
        };
        let sequence = matches!(child.kind, NodeKind::BlockSequence(_));
        if sequence
            && items.last().is_some_and(|item| {
                !item.explicit_key
                    && item
                        .sep
                        .as_ref()
                        .is_some_and(|sep| !includes_token(sep, K::Newline))
            })
        {
            self.stack.push(node);
            self.attach(Node::new(
                NodeKind::Error("Unexpected block sequence on same line with key"),
                &self.token,
            ));
            return false;
        }
        if !sequence && at_indent {
            items.push(CollectionItem {
                start,
                ..CollectionItem::default()
            });
        }
        self.stack.push(node);
        self.stack.push(child);
        false
    }
}
