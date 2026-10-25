//! Borrowed authored schema nodes, their declared kinds and their definitions.

use std::{borrow::Cow, collections::HashMap, rc::Rc};

use serde_json::Value;

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
    pub(crate) fn new(root: &'a Value, kinds: &'a HashMap<String, SchemaKind>) -> Self {
        let indexed = index(root, kinds);
        Self {
            root,
            kinds: indexed,
        }
    }
}

impl<'a> Document<'a> {
    /// The authored view of the supplied schema document's root.
    pub(crate) fn root(&'a self) -> View<'a> {
        self.view(self.root)
    }

    /// The authored view of one supplied node, with its declared kind when it has one.
    pub(crate) fn view(&'a self, raw: &'a Value) -> View<'a> {
        View::at(raw, &self.kinds)
    }
}
/// Borrowed authored schema node with its declared kind.
#[derive(Clone, Copy)]
pub(crate) struct View<'a> {
    /// Original JSON node the author supplied.
    pub(crate) raw: &'a Value,
    /// Kind the author declared for this node, when the node is authored.
    pub(crate) kind: Option<&'a SchemaKind>,
    /// Indexed kinds a child view looks its own declared kind up in.
    kinds: &'a HashMap<*const Value, &'a SchemaKind>,
}

impl<'a> View<'a> {
    /// The authored view of one node, looked up in the supplied declared-kind index.
    fn at(raw: &'a Value, kinds: &'a HashMap<*const Value, &'a SchemaKind>) -> Self {
        Self {
            raw,
            kind: kinds.get(&std::ptr::from_ref(raw)).copied(),
            kinds,
        }
    }

    /// The authored view of one child node of this node.
    pub(crate) fn child(self, raw: &'a Value) -> View<'a> {
        Self::at(raw, self.kinds)
    }

    /// The authored view of the node this keyword addresses, when it holds one.
    pub(crate) fn keyword(self, keyword: &str) -> Option<View<'a>> {
        self.raw.get(keyword).map(|raw| self.child(raw))
    }

    /// The authored views of every node this keyword lists.
    pub(crate) fn children(self, keyword: &str) -> Option<Vec<View<'a>>> {
        Some(
            super::super::references::children(self.raw.get(keyword)?)
                .into_iter()
                .map(|raw| self.child(raw))
                .collect(),
        )
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
    /// Original authored node retained by an instantiated or composed stage.
    pub(crate) fn source(&self) -> Option<View<'a>> {
        match self {
            Self::Leaf(view) => Some(*view),
            Self::Object(object) => object.source,
            Self::Array(_, source)
            | Self::Tuple(_, source)
            | Self::Union(_, source)
            | Self::Intersect(_, source) => *source,
            _ => None,
        }
    }
}

impl Node<'_> {
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
}

/// Resolve supplied node pointers once, retaining the original schema identities.
pub(in super::super) fn index<'a, T>(
    root: &'a Value,
    entries: &'a HashMap<String, T>,
) -> HashMap<*const Value, &'a T> {
    entries
        .iter()
        .filter_map(|(pointer, item)| Some((std::ptr::from_ref(root.pointer(pointer)?), item)))
        .collect()
}
