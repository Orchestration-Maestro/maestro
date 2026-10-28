//! Resolve concrete syntax into owned metadata with completed anchors.

pub(super) mod collection;

use super::{
    FrontmatterError, FrontmatterValue,
    alias_count::Anchors,
    cst::{Node, NodeKind, SourceToken, TokenKind as K},
    parser,
    resolve_flow_scalar::Style,
    resolve_props::{Properties, PropertyContext},
    values,
};
use collection::{Child, CollectionPlan, Work};

/// One document's value resolution and source-coordinate formatting.
pub(super) struct Composer<'a> {
    /// Document text, accessed only through token ranges or error coordinates.
    source: &'a str,
    /// Completed anchor bindings and expansion weights.
    pub anchors: Anchors,
}

/// Compose one YAML document, retaining an absent or null root.
pub(super) fn compose(source: &str) -> Result<FrontmatterValue, FrontmatterError> {
    let nodes = parser::parse(source)?;
    let mut composer = Composer {
        source,
        anchors: Anchors::default(),
    };
    let mut value = None;
    for node in &nodes {
        match &node.kind {
            NodeKind::Document {
                start,
                value: content,
            } => {
                if value.is_some() {
                    return Err(
                        composer.error_at(node.offset, "Source contains multiple documents")
                    );
                }
                let props = composer.resolve_props(
                    start,
                    PropertyContext {
                        indicator: K::DocStart,
                        flow: false,
                        newline: true,
                        indent: 0,
                    },
                    content.as_deref(),
                )?;
                value = Some(composer.compose_doc(content.as_deref(), props)?);
                composer.resolve_end(&node.end, true)?;
            }
            NodeKind::DocEnd => composer.resolve_end(&node.end, true)?,
            NodeKind::Error(message) => return Err(composer.error_at(node.offset, message)),
            _ => return Err(composer.error_at(node.offset, "Unexpected document content")),
        }
    }
    Ok(value.unwrap_or(FrontmatterValue::Null))
}

impl<'a> Composer<'a> {
    /// Read only the byte range owned by one token.
    pub fn text(&self, token: &SourceToken) -> &'a str {
        &self.source[token.range()]
    }

    /// Attach byte, line and column coordinates to an error.
    pub fn error_at(&self, offset: usize, message: &str) -> FrontmatterError {
        let prefix = &self.source[..offset];
        let eof_line = offset == self.source.len() && !prefix.is_empty() && !prefix.ends_with('\n');
        let line = prefix.bytes().filter(|b| *b == b'\n').count() + 1 + usize::from(eof_line);
        let column = if eof_line {
            1
        } else {
            prefix
                .rsplit('\n')
                .next()
                .unwrap_or_default()
                .chars()
                .count()
                + 1
        };
        FrontmatterError {
            message: format!("{message} at byte {offset} line {line} column {column}"),
            line: Some(line),
            column: Some(column),
        }
    }

    /// Check an implicit key's supplied span against the Unicode character limit.
    pub fn implicit_key_too_long(
        &self,
        key: Option<&Node>,
        props: &Properties,
        colon: &SourceToken,
    ) -> bool {
        let start = if props.anchor.is_some() || props.tag.is_some() {
            props.start
        } else {
            key.map_or(colon.offset, |key| key.offset)
        };
        self.source[start..colon.offset].chars().nth(1024).is_some()
    }

    /// Compose children before parents without using the native call stack for nesting.
    fn compose_doc(
        &mut self,
        node: Option<&Node>,
        props: Properties,
    ) -> Result<FrontmatterValue, FrontmatterError> {
        let mut work = vec![Work::Node(node, props)];
        let mut values = Vec::new();
        while let Some(next) = work.pop() {
            match next {
                Work::Node(node, props) => {
                    self.compose_work_node(node, props, &mut work, &mut values)?;
                }
                Work::Finish {
                    kind,
                    anchor,
                    mark,
                    base,
                } => {
                    let value = kind.finish(values.split_off(base))?;
                    self.publish(anchor, &value, mark);
                    values.push(value);
                }
            }
        }
        values
            .pop()
            .ok_or_else(|| FrontmatterError::new("Missing document value"))
    }

    /// Schedule a collection continuation or resolve a leaf immediately.
    fn compose_work_node<'n>(
        &mut self,
        node: Option<&'n Node>,
        props: Properties,
        work: &mut Vec<Work<'n>>,
        values: &mut Vec<FrontmatterValue>,
    ) -> Result<(), FrontmatterError> {
        let mark = self.anchors.mark();
        if let Some(node) = node.filter(|node| node.is_collection()) {
            let plan = self.compose_collection(node, &props)?;
            work.push(Work::Finish {
                kind: plan.kind,
                anchor: props.anchor,
                mark,
                base: values.len(),
            });
            work.extend(
                plan.children
                    .into_iter()
                    .rev()
                    .map(|child| Work::Node(child.node, child.props)),
            );
        } else {
            let value = self.compose_node(node, &props)?;
            self.publish(props.anchor, &value, mark);
            values.push(value);
        }
        Ok(())
    }

    /// Prepare a child, propagating concrete parser errors when reached.
    pub fn child<'n>(
        &self,
        node: Option<&'n Node>,
        props: Properties,
    ) -> Result<Child<'n>, FrontmatterError> {
        if let Some(node) = node {
            self.node_error(node)?;
        }
        Ok(Child { node, props })
    }

    /// Report a concrete syntax error at its producer's token position.
    fn node_error(&self, node: &Node) -> Result<(), FrontmatterError> {
        if let NodeKind::Error(message) = node.kind {
            return Err(self.error_at(node.offset, message));
        }
        Ok(())
    }

    /// Publish a completed binding only when an anchor was authored.
    fn publish(&mut self, anchor: Option<String>, value: &FrontmatterValue, mark: usize) {
        if let Some(anchor) = anchor {
            self.anchors.publish(anchor, value.clone(), mark);
        }
    }

    /// Resolve one leaf node or an absent scalar.
    fn compose_node(
        &mut self,
        node: Option<&Node>,
        props: &Properties,
    ) -> Result<FrontmatterValue, FrontmatterError> {
        let Some(node) = node else {
            self.anchors.leaf();
            return Ok(values::scalar(
                String::new(),
                Style::Plain,
                props.tag.as_ref(),
            ));
        };
        self.node_error(node)?;
        match &node.kind {
            NodeKind::Scalar(token) if token.kind == K::Alias => {
                if props.anchor.is_some() || props.tag.is_some() {
                    return Err(self.error_at(node.offset, "Alias cannot have properties"));
                }
                self.resolve_end(&node.end, false)?;
                self.anchors
                    .resolve(self.text(token).strip_prefix('*').unwrap_or_default())
            }
            NodeKind::Scalar(token) => {
                let (text, style) = self.resolve_flow_scalar(token)?;
                self.scalar_end(node, token)?;
                self.anchors.leaf();
                Ok(values::scalar(text, style, props.tag.as_ref()))
            }
            NodeKind::BlockScalar {
                props: header,
                source,
            } => {
                let text = self.resolve_block_scalar(node, header, source.as_ref())?;
                self.anchors.leaf();
                Ok(values::scalar(text, Style::Quoted, props.tag.as_ref()))
            }
            _ => Err(self.error_at(node.offset, "Unexpected document node")),
        }
    }

    /// Preserve quoted-scalar trailing-content errors while validating all end tokens.
    fn scalar_end(&self, node: &Node, token: &SourceToken) -> Result<(), FrontmatterError> {
        if matches!(token.kind, K::SingleQuotedScalar | K::DoubleQuotedScalar)
            && let Some(trailing) = node.end.iter().find(|token| !token.kind.is_trivia())
        {
            let kind = if token.kind == K::SingleQuotedScalar {
                "single"
            } else {
                "double"
            };
            return Err(self.error_at(
                trailing.offset,
                &format!("invalid trailing content after {kind}-quoted scalar"),
            ));
        }
        self.resolve_end(&node.end, false)
    }

    /// Validate collection syntax and schedule each child in authored order.
    fn compose_collection<'n>(
        &self,
        node: &'n Node,
        props: &Properties,
    ) -> Result<CollectionPlan<'n>, FrontmatterError> {
        if (props.anchor.is_some() || props.tag.is_some())
            && matches!(node.kind, NodeKind::BlockSequence(_))
            && !props.newline_after_prop
        {
            return Err(self.error_at(
                node.offset,
                "Missing newline after block sequence properties",
            ));
        }
        match &node.kind {
            NodeKind::BlockMap(items) => self.resolve_block_map(node, items),
            NodeKind::BlockSequence(items) => self.resolve_block_seq(node, items),
            NodeKind::FlowCollection { start, items } => {
                self.resolve_flow_collection(node, start, items)
            }
            _ => Err(self.error_at(node.offset, "Expected collection")),
        }
    }
}
