//! Borrowed authored schema nodes, their declared kinds and their definitions.

use std::{borrow::Cow, collections::HashMap, rc::Rc};

use serde_json::{Map, Value};

use crate::SchemaKind;

/// Authored kinds indexed by the identity of the JSON node they describe.
///
/// A JSON pointer stays an address in the supplied document; it is never used to rebuild a
/// schema node after composition. Resolving every pointer once keeps node identity the only
/// handle conversion, instantiation and evaluation pass around.
pub(crate) struct Document<'a> {
    /// Supplied schema value every borrowed node belongs to.
    root: &'a Value,
    /// Declared kind of each authored node, by that node's address.
    kinds: HashMap<*const Value, &'a SchemaKind>,
}

impl<'a> Document<'a> {
    /// Index each authored kind by the node its pointer addresses.
    fn new(root: &'a Value, kinds: &'a HashMap<String, SchemaKind>) -> Self {
        let indexed = kinds
            .iter()
            .filter_map(|(pointer, kind)| {
                Some((std::ptr::from_ref::<Value>(root.pointer(pointer)?), kind))
            })
            .collect();
        Self {
            root,
            kinds: indexed,
        }
    }
}

/// Borrowed authored schema node with its declared kind.
#[derive(Clone, Copy)]
pub(crate) struct View<'a> {
    /// Original JSON node the author supplied.
    pub(crate) raw: &'a Value,
    /// Kind the author declared for this node, when the node is authored.
    pub(crate) kind: Option<&'a SchemaKind>,
}

/// One authored conversion: the supplied document, its kinds and its definition frames.
pub(crate) struct Converter<'a> {
    /// Authored kinds by node identity.
    document: Document<'a>,
    /// Definition frames on the conversion call path, outermost first; each frame's enclosing
    /// frame is the one below it, and a frame leaves when the conversion that entered it returns.
    frames: Vec<&'a Map<String, Value>>,
}

impl<'a> Converter<'a> {
    /// Start converting against the supplied document.
    pub(crate) fn new(root: &'a Value, kinds: &'a HashMap<String, SchemaKind>) -> Self {
        Self {
            document: Document::new(root, kinds),
            frames: Vec::new(),
        }
    }

    /// The view of the supplied document root.
    pub(crate) fn root(&self) -> View<'a> {
        self.view(self.document.root)
    }

    /// The view of one node of the supplied document.
    pub(crate) fn view(&self, raw: &'a Value) -> View<'a> {
        View {
            raw,
            kind: self
                .document
                .kinds
                .get(&std::ptr::from_ref::<Value>(raw))
                .copied(),
        }
    }

    /// The child node a keyword names, when the supplied view holds it.
    pub(crate) fn keyword(&self, view: View<'a>, keyword: &str) -> Option<View<'a>> {
        view.raw.get(keyword).map(|child| self.view(child))
    }

    /// The child nodes a keyword names, the members of an array or the values of an object.
    pub(crate) fn children(&self, view: View<'a>, keyword: &str) -> Option<Vec<View<'a>>> {
        let children = super::super::references::children(view.raw.get(keyword)?);
        Some(children.into_iter().map(|child| self.view(child)).collect())
    }

    /// The definition a name names in the current frames, the innermost declaration first.
    pub(crate) fn definition(&self, name: &str) -> Option<View<'a>> {
        super::super::references::definition(&self.frames, name).map(|raw| self.view(raw))
    }

    /// The current definition frames, outermost first.
    pub(crate) fn frames(&self) -> &[&'a Map<String, Value>] {
        &self.frames
    }

    /// Run one conversion step with the definitions a cyclic node declares visible.
    ///
    /// The declarations overlay the current frames for that step alone, so a sibling converted
    /// afterwards sees the enclosing frames again.
    pub(crate) fn within<R>(&mut self, view: View<'a>, step: impl FnOnce(&mut Self) -> R) -> R {
        let depth = self.frames.len();
        self.frames
            .extend(view.raw.get("$defs").and_then(Value::as_object));
        let result = step(self);
        self.frames.truncate(depth);
        result
    }
}

#[cfg(test)]
thread_local! {
    /// Member lookups performed on this thread, read by the work-bound tests.
    pub(crate) static MEMBER_LOOKUPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// One schema node as an authored view or a composed stage result.
///
/// A composed sequence, alternative set or intersection keeps the authored node it was
/// instantiated from, which still carries every option and modifier its author declared.
pub(crate) enum Node<'a> {
    /// Borrowed authored node with its declared kind.
    Leaf(View<'a>),
    /// Single constant value alternative.
    Constant(Cow<'a, Value>),
    /// The JSON null value.
    Null,
    /// Object whose members were instantiated or composed.
    Object(ObjectNode<'a>),
    /// Sequence whose members share one node.
    Array(Rc<Node<'a>>, Option<View<'a>>),
    /// Sequence whose positions have their own nodes.
    Tuple(Vec<Rc<Node<'a>>>, Option<View<'a>>),
    /// Alternative members.
    Union(Vec<Rc<Node<'a>>>, Option<View<'a>>),
    /// Intersected members.
    Intersect(Vec<Rc<Node<'a>>>, Option<View<'a>>),
    /// Accepts no value.
    Never,
    /// Accepts every value.
    Any,
    /// Accepts every JSON value, asked as the reference's unknown type.
    Unknown,
}

/// Composed object members in composition order.
pub(crate) struct ObjectNode<'a> {
    /// Members and their instantiated nodes.
    pub(crate) properties: Vec<Member<'a>>,
    /// Authored object the members were instantiated from, when there is one.
    pub(crate) source: Option<View<'a>>,
}

/// One named member of an object-like node.
#[derive(Clone)]
pub(crate) struct Member<'a> {
    /// Exact member name.
    pub(crate) name: Cow<'a, str>,
    /// Node the member converts and compares through.
    pub(crate) node: Rc<Node<'a>>,
    /// Whether a value may leave the member out.
    pub(crate) optional: bool,
}

impl<'a> Member<'a> {
    /// The members an authored object declares, in declaration order, each through the node
    /// the supplied stage gives its child; a member the object does not require is optional.
    pub(crate) fn declared(
        converter: &Converter<'a>,
        view: View<'a>,
        mut node: impl FnMut(View<'a>) -> Rc<Node<'a>>,
    ) -> Option<Vec<Self>> {
        let properties = view.raw.get("properties").and_then(Value::as_object)?;
        let required = view.raw.get("required").and_then(Value::as_array);
        Some(
            properties
                .iter()
                .map(|(name, child)| Self {
                    name: Cow::Borrowed(name.as_str()),
                    node: node(converter.view(child)),
                    optional: !required.is_some_and(|required| {
                        required
                            .iter()
                            .any(|required| required.as_str() == Some(name))
                    }),
                })
                .collect(),
        )
    }
}

/// Object-like members indexed once by their exact names.
pub(crate) struct Members<'m, 'a>(HashMap<&'m str, &'m Member<'a>>);

impl<'m, 'a> Members<'m, 'a> {
    /// Index the supplied members by name.
    pub(crate) fn new(members: &'m [Member<'a>]) -> Self {
        Self(
            members
                .iter()
                .map(|member| (member.name.as_ref(), member))
                .collect(),
        )
    }

    /// The member the supplied name names.
    pub(crate) fn get(&self, name: &str) -> Option<&'m Member<'a>> {
        #[cfg(test)]
        MEMBER_LOOKUPS.with(|count| count.set(count.get() + 1));
        self.0.get(name).copied()
    }
}

impl<'a> Node<'a> {
    /// The kind this node converts and compares as.
    pub(crate) fn kind(&self) -> Option<SchemaKind> {
        match self {
            Node::Leaf(view) => view.kind.cloned(),
            Node::Constant(_) => Some(SchemaKind::Literal),
            Node::Null => Some(SchemaKind::Null),
            Node::Object(_) => Some(SchemaKind::Object),
            Node::Array(..) => Some(SchemaKind::Array),
            Node::Tuple(..) => Some(SchemaKind::Tuple),
            Node::Union(..) => Some(SchemaKind::Union),
            Node::Intersect(..) => Some(SchemaKind::Intersect),
            Node::Never => Some(SchemaKind::Never),
            Node::Any => Some(SchemaKind::Any),
            Node::Unknown => Some(SchemaKind::Unknown),
        }
    }

    /// The parts of a union, tuple or intersection: the nodes a composed one holds, or the
    /// children its authored node declares, each with its declared kind.
    pub(crate) fn parts(&self, converter: &Converter<'a>) -> Option<Cow<'_, [Rc<Node<'a>>]>> {
        let view = match self {
            Node::Union(parts, _) | Node::Tuple(parts, _) | Node::Intersect(parts, _) => {
                return Some(Cow::Borrowed(parts));
            }
            Node::Leaf(view) => *view,
            _ => return None,
        };
        let keyword = match view.kind? {
            SchemaKind::Union => "anyOf",
            SchemaKind::Tuple => "items",
            SchemaKind::Intersect => "allOf",
            _ => return None,
        };
        let children = converter.children(view, keyword)?;
        Some(Cow::Owned(
            children
                .into_iter()
                .map(|child| Rc::new(Node::Leaf(child)))
                .collect(),
        ))
    }

    /// The members of an object-like node, object or tuple, in member order.
    ///
    /// A tuple's positions become required members named by their index.
    pub(crate) fn members(&self) -> Option<Cow<'_, [Member<'a>]>> {
        match self {
            Node::Object(object) => Some(Cow::Borrowed(&object.properties)),
            Node::Tuple(items, _) => Some(Cow::Owned(
                items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| Member {
                        name: Cow::Owned(index.to_string()),
                        node: Rc::clone(item),
                        optional: false,
                    })
                    .collect(),
            )),
            _ => None,
        }
    }
}
