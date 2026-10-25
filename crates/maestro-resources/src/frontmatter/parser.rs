//! Iterative construction of document syntax from lexical tokens.

mod block_map;
mod flow_collection;

use super::{
    FrontmatterError,
    cst::{
        CollectionItem, Node, NodeKind, SourceToken, TokenKind as K, get_first_key_start_props,
        includes_token,
    },
    lexer::Lexer,
};

/// One document stream's syntax construction state.
pub(super) struct Parser {
    /// Nodes under construction, with at most one thousand open collections.
    stack: Vec<Node>,
    /// Completed stream nodes.
    output: Vec<Node>,
    /// Current lexical unit.
    token: SourceToken,
    /// Whether indentation is still accumulating.
    at_new_line: bool,
    /// Current indentation from lexical tokens.
    indent: usize,
    /// Whether a block mapping key shares this line.
    on_key_line: bool,
}

/// Build complete syntax without resolving values.
pub(super) fn parse(source: &str) -> Result<Vec<Node>, FrontmatterError> {
    let mut parser = Parser {
        stack: Vec::new(),
        output: Vec::new(),
        token: SourceToken::new(K::DocMode, 0..0),
        at_new_line: true,
        indent: 0,
        on_key_line: false,
    };
    for mut token in Lexer::lex(source) {
        if token.kind == K::UnclosedQuotedScalar {
            return Ok(vec![Node::new(
                NodeKind::Error("Unclosed quoted scalar"),
                &token,
            )]);
        }
        token.indent = parser.indent;
        parser.token = token;
        if parser.token.kind == K::Scalar {
            parser.at_new_line = false;
        }
        while parser.step() {}
        if parser.stack.len() > 1000
            && parser
                .stack
                .iter()
                .filter(|node| node.is_collection())
                .count()
                > 1000
        {
            return Err(FrontmatterError::new(
                "Collections nest deeper than 1000 levels",
            ));
        }
        parser.next(source);
    }
    while let Some(node) = parser.stack.pop() {
        parser.attach(node);
    }
    Ok(parser.output)
}

impl Parser {
    /// Update line state using only the current token's range.
    fn next(&mut self, source: &str) {
        match self.token.kind {
            K::Newline => {
                self.at_new_line = true;
                self.indent = 0;
            }
            K::Space
                if self.at_new_line
                    && source
                        .get(self.token.range())
                        .is_some_and(|text| text.starts_with(' ')) =>
            {
                self.indent += self.token.end - self.token.offset;
            }
            K::SeqItemInd | K::ExplicitKeyInd | K::MapValueInd if self.at_new_line => {
                self.indent += self.token.end - self.token.offset;
            }
            K::DocMode | K::FlowErrorEnd | K::Scalar => (),
            _ => self.at_new_line = false,
        }
    }

    /// Consume one token or settle a node before retrying that token.
    fn step(&mut self) -> bool {
        if self.token.kind == K::DocEnd
            && !matches!(self.stack.last().map(|n| &n.kind), Some(NodeKind::DocEnd))
        {
            while let Some(node) = self.stack.pop() {
                self.attach(node);
            }
            self.stack.push(Node::new(NodeKind::DocEnd, &self.token));
            return false;
        }
        let Some(node) = self.stack.pop() else {
            self.stream();
            return false;
        };
        match node.kind {
            NodeKind::Document { .. } => self.document(node),
            NodeKind::Scalar(_) => self.scalar(node),
            NodeKind::BlockScalar { .. } => self.block_scalar(node),
            NodeKind::BlockMap(_) => self.block_map(node),
            NodeKind::BlockSequence(_) => self.block_sequence(node),
            NodeKind::FlowCollection { .. } => self.flow_collection(node),
            NodeKind::DocEnd => {
                let mut node = node;
                if self.token.kind != K::DocMode {
                    node.end.push(self.token.clone());
                }
                if self.token.kind == K::Newline {
                    self.attach(node);
                } else {
                    self.stack.push(node);
                }
                false
            }
            NodeKind::Error(_) => {
                self.attach(node);
                true
            }
        }
    }

    /// Interpret a token outside a document.
    fn stream(&mut self) {
        match self.token.kind {
            K::DocMode | K::DocStart => {
                let start = if self.token.kind == K::DocStart {
                    vec![self.token.clone()]
                } else {
                    Vec::new()
                };
                self.stack.push(Node::new(
                    NodeKind::Document { start, value: None },
                    &self.token,
                ));
            }
            K::ByteOrderMark | K::Space | K::Comment | K::Newline => (),
            _ => self.output.push(Node::new(
                NodeKind::Error("Unexpected token in document stream"),
                &self.token,
            )),
        }
    }

    /// Retain document properties until the first content node.
    fn document(&mut self, mut node: Node) -> bool {
        let NodeKind::Document { start, value } = &mut node.kind else {
            return false;
        };
        if value.is_some() {
            return self.line_end(node);
        }
        if matches!(
            self.token.kind,
            K::Anchor | K::Tag | K::Space | K::Comment | K::Newline
        ) || self.token.kind == K::DocStart && start.iter().all(|t| t.kind.is_trivia())
        {
            start.push(self.token.clone());
            self.stack.push(node);
            return false;
        }
        let child = self.start_block_value(&mut node);
        self.stack.push(node);
        if let Some(child) = child {
            self.stack.push(child);
        } else {
            self.attach(Node::new(
                NodeKind::Error("Unexpected token in document"),
                &self.token,
            ));
        }
        false
    }

    /// Turn a scalar followed by a colon into a mapping key.
    fn scalar(&mut self, mut node: Node) -> bool {
        if self.token.kind != K::MapValueInd {
            return self.line_end(node);
        }
        let start = self
            .previous_props()
            .map(get_first_key_start_props)
            .unwrap_or_default();
        let mut sep = std::mem::take(&mut node.end);
        sep.push(self.token.clone());
        let offset = node.offset;
        let indent = node.indent;
        let item = CollectionItem {
            start,
            key: Some(Box::new(node)),
            sep: Some(sep),
            ..CollectionItem::default()
        };
        self.stack.push(Node {
            kind: NodeKind::BlockMap(vec![item]),
            offset,
            indent,
            end: Vec::new(),
        });
        self.on_key_line = true;
        false
    }

    /// Attach the lexer's complete scalar body to its header node.
    fn block_scalar(&mut self, mut node: Node) -> bool {
        let NodeKind::BlockScalar { props, source } = &mut node.kind else {
            return false;
        };
        match self.token.kind {
            K::Space | K::Comment | K::Newline | K::Error => {
                props.push(self.token.clone());
                self.stack.push(node);
                false
            }
            K::Scalar => {
                *source = Some(self.token.clone());
                self.at_new_line = true;
                self.indent = 0;
                self.attach(node);
                false
            }
            _ => {
                self.attach(node);
                true
            }
        }
    }

    /// Start a node from one token, moving only its parent's key properties.
    fn start_block_value(&mut self, parent: &mut Node) -> Option<Node> {
        let kind = match self.token.kind {
            k if k.is_scalar() => NodeKind::Scalar(self.token.clone()),
            K::BlockScalarHeader => NodeKind::BlockScalar {
                props: vec![self.token.clone()],
                source: None,
            },
            K::FlowMapStart | K::FlowSeqStart => NodeKind::FlowCollection {
                start: self.token.clone(),
                items: Vec::new(),
            },
            K::SeqItemInd => NodeKind::BlockSequence(vec![CollectionItem {
                start: vec![self.token.clone()],
                ..CollectionItem::default()
            }]),
            K::ExplicitKeyInd | K::MapValueInd => {
                self.on_key_line = true;
                let mut start = Self::node_props(parent)
                    .map(get_first_key_start_props)
                    .unwrap_or_default();
                let explicit_key = self.token.kind == K::ExplicitKeyInd;
                if explicit_key {
                    start.push(self.token.clone());
                }
                let sep = (!explicit_key).then(|| vec![self.token.clone()]);
                NodeKind::BlockMap(vec![CollectionItem {
                    start,
                    sep,
                    explicit_key,
                    ..CollectionItem::default()
                }])
            }
            _ => return None,
        };
        Some(Node::new(kind, &self.token))
    }

    /// Property list immediately preceding the next node.
    fn previous_props(&mut self) -> Option<&mut Vec<SourceToken>> {
        self.stack.last_mut().and_then(Self::node_props)
    }

    /// Property list owned by a block parent.
    fn node_props(node: &mut Node) -> Option<&mut Vec<SourceToken>> {
        match &mut node.kind {
            NodeKind::Document { start, .. } => Some(start),
            NodeKind::BlockMap(items) => items
                .last_mut()
                .map(|item| item.sep.as_mut().unwrap_or(&mut item.start)),
            NodeKind::BlockSequence(items) => items.last_mut().map(|item| &mut item.start),
            _ => None,
        }
    }

    /// Settle a completed node into its parent and hoist trailing comment-only items.
    fn attach(&mut self, mut node: Node) {
        let Some(parent) = self.stack.last_mut() else {
            self.output.push(node);
            return;
        };
        if matches!(node.kind, NodeKind::BlockScalar { .. }) {
            node.indent = parent.indent;
        }
        if matches!(node.kind, NodeKind::FlowCollection { .. }) {
            if matches!(parent.kind, NodeKind::Document { .. }) {
                node.indent = 0;
            }
            Self::fix_flow_seq_items(&mut node);
        }
        let tail = Self::hoist_comments(&mut node);
        match &mut parent.kind {
            NodeKind::Document { value, .. } => {
                *value = Some(Box::new(node));
                if let Some(tail) = tail {
                    parent.end = tail;
                }
            }
            NodeKind::BlockMap(items) | NodeKind::FlowCollection { items, .. } => {
                if let Some(on_key_line) = Self::attach_mapping(items, node) {
                    self.on_key_line = on_key_line;
                }
                if let Some(tail) = tail {
                    items.push(CollectionItem {
                        start: tail,
                        ..CollectionItem::default()
                    });
                }
            }
            NodeKind::BlockSequence(items) => {
                if let Some(item) = items.last_mut().filter(|item| item.value.is_none()) {
                    item.value = Some(Box::new(node));
                } else {
                    items.push(CollectionItem {
                        value: Some(Box::new(node)),
                        ..CollectionItem::default()
                    });
                }
                if let Some(tail) = tail {
                    items.push(CollectionItem {
                        start: tail,
                        ..CollectionItem::default()
                    });
                }
            }
            _ => self.output.push(Node::new(
                NodeKind::Error("Unexpected nested node"),
                &self.token,
            )),
        }
    }

    /// Attach a completed mapping key or value using separator presence.
    fn attach_mapping(items: &mut Vec<CollectionItem>, node: Node) -> Option<bool> {
        let Some(item) = items.last_mut().filter(|item| item.value.is_none()) else {
            items.push(CollectionItem {
                key: Some(Box::new(node)),
                sep: Some(Vec::new()),
                ..CollectionItem::default()
            });
            return Some(true);
        };
        if item.sep.is_some() {
            item.value = Some(Box::new(node));
            return None;
        }
        item.key = Some(Box::new(node));
        item.sep = Some(Vec::new());
        Some(!item.explicit_key)
    }

    /// Move a trailing unindented comment-only entry out of a block collection.
    fn hoist_comments(node: &mut Node) -> Option<Vec<SourceToken>> {
        let (NodeKind::BlockMap(items) | NodeKind::BlockSequence(items)) = &mut node.kind else {
            return None;
        };
        let last = items.last()?;
        if last.sep.is_some()
            || last.value.is_some()
            || last.start.is_empty()
            || !last.start.iter().all(|t| t.kind.is_trivia())
        {
            return None;
        }
        if node.indent != 0
            && last
                .start
                .iter()
                .any(|t| t.kind == K::Comment && t.indent >= node.indent)
        {
            return None;
        }
        items.pop().map(|item| item.start)
    }

    /// Convert separator-only flow sequence entries back to ordinary values.
    fn fix_flow_seq_items(node: &mut Node) {
        let NodeKind::FlowCollection { start, items } = &mut node.kind else {
            return;
        };
        if start.kind != K::FlowSeqStart {
            return;
        }
        for item in items {
            if item.sep.is_none()
                || item.value.is_some()
                || includes_token(&item.start, K::ExplicitKeyInd)
                || item
                    .sep
                    .as_ref()
                    .is_some_and(|sep| includes_token(sep, K::MapValueInd))
            {
                continue;
            }
            item.value = item.key.take();
            let Some(mut sep) = item.sep.take() else {
                continue;
            };
            let target = item
                .value
                .as_mut()
                .filter(|value| value.is_flow())
                .map_or(&mut item.start, |value| &mut value.end);
            target.append(&mut sep);
        }
    }

    /// Retain trailing tokens, or finish a node before a structural delimiter.
    fn line_end(&mut self, mut node: Node) -> bool {
        if matches!(
            self.token.kind,
            K::Comma | K::DocStart | K::DocEnd | K::FlowSeqEnd | K::FlowMapEnd | K::MapValueInd
        ) {
            self.attach(node);
            return true;
        }
        node.end.push(self.token.clone());
        if self.token.kind == K::Newline {
            self.on_key_line = false;
            self.attach(node);
        } else {
            self.stack.push(node);
        }
        false
    }

    /// Attach whitespace to the current entry or to an indented preceding value.
    fn collection_trivia(&mut self, node: &mut Node) {
        let (NodeKind::BlockMap(items) | NodeKind::BlockSequence(items)) = &mut node.kind else {
            return;
        };
        let has_previous = items.len() >= 2;
        let Some(item) = items.last_mut() else {
            return;
        };
        if item.value.is_some() {
            if self.token.kind == K::Newline
                && let Some(value) = &mut item.value
                && value.end.last().is_some_and(|t| t.kind == K::Comment)
            {
                value.end.push(self.token.clone());
            } else {
                items.push(CollectionItem {
                    start: vec![self.token.clone()],
                    ..CollectionItem::default()
                });
            }
        } else if let Some(sep) = &mut item.sep {
            sep.push(self.token.clone());
        } else if self.token.kind == K::Comment
            && self.token.indent > node.indent
            && item
                .start
                .iter()
                .all(|t| matches!(t.kind, K::Space | K::Newline))
            && has_previous
        {
            let tail = items.pop().map(|item| item.start).unwrap_or_default();
            if let Some(value) = items.last_mut().and_then(|item| item.value.as_mut()) {
                value.end.extend(tail);
                value.end.push(self.token.clone());
            }
        } else {
            item.start.push(self.token.clone());
        }
    }

    /// Place sequence indicators and children using parser indentation only.
    fn block_sequence(&mut self, mut node: Node) -> bool {
        if self.token.kind.is_trivia() {
            self.collection_trivia(&mut node);
            self.stack.push(node);
            return false;
        }
        let NodeKind::BlockSequence(items) = &mut node.kind else {
            return false;
        };
        let Some(item) = items.last_mut() else {
            return false;
        };
        match self.token.kind {
            K::Anchor | K::Tag if item.value.is_none() && self.token.indent > node.indent => {
                item.start.push(self.token.clone());
                self.stack.push(node);
                return false;
            }
            K::SeqItemInd if self.token.indent == node.indent => {
                if item.value.is_some() || includes_token(&item.start, K::SeqItemInd) {
                    items.push(CollectionItem {
                        start: vec![self.token.clone()],
                        ..CollectionItem::default()
                    });
                } else {
                    item.start.push(self.token.clone());
                }
                self.stack.push(node);
                return false;
            }
            _ => (),
        }
        if self.token.indent > node.indent
            && let Some(child) = self.start_block_value(&mut node)
        {
            self.stack.push(node);
            self.stack.push(child);
            return false;
        }
        self.attach(node);
        true
    }
}
