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

    /// The view of a node seen through the definitions visible at that point.
    fn view(&self, raw: &'a Value, visible: usize) -> View<'a> {
        View {
            raw,
            kind: self.kinds.get(&std::ptr::from_ref::<Value>(raw)).copied(),
            visible,
        }
    }
}

/// Borrowed authored schema node with its declared kind and visible definitions.
#[derive(Clone, Copy)]
pub(crate) struct View<'a> {
    /// Original JSON node the author supplied.
    pub(crate) raw: &'a Value,
    /// Kind the author declared for this node, when the node is authored.
    pub(crate) kind: Option<&'a SchemaKind>,
    /// Definitions this node can see, as a prefix of the operation's definition stack.
    pub(crate) visible: usize,
}

/// One definition an enclosing cyclic node made visible.
struct Definition<'a> {
    /// Declaration name the `$ref` member names.
    name: &'a str,
    /// Definition node and the definitions visible inside it.
    view: View<'a>,
}

/// One authored conversion: the supplied document, its kinds and visible definitions.
pub(crate) struct Converter<'a> {
    /// Authored kinds by node identity.
    document: Document<'a>,
    /// Definitions entered so far, outermost first; entries are only ever appended, so a
    /// captured prefix keeps naming the declarations its scope saw.
    definitions: Vec<Definition<'a>>,
}

impl<'a> Converter<'a> {
    /// Start converting against the supplied document.
    pub(crate) fn new(root: &'a Value, kinds: &'a HashMap<String, SchemaKind>) -> Self {
        Self {
            document: Document::new(root, kinds),
            definitions: Vec::new(),
        }
    }

    /// The view of the supplied document root.
    pub(crate) fn root(&self) -> View<'a> {
        self.document.view(self.document.root, 0)
    }

    /// The view of a child node of the supplied view.
    pub(crate) fn child(&self, parent: View<'a>, raw: &'a Value) -> View<'a> {
        self.document.view(raw, parent.visible)
    }

    /// The child node a keyword names, when the supplied view holds it.
    pub(crate) fn keyword(&self, view: View<'a>, keyword: &str) -> Option<View<'a>> {
        view.raw
            .get(keyword)
            .map(|child| self.document.view(child, view.visible))
    }

    /// The definition a name visible at the supplied view names.
    pub(crate) fn definition(&self, view: View<'a>, name: &str) -> Option<View<'a>> {
        self.definitions[..view.visible]
            .iter()
            .rev()
            .find(|definition| definition.name == name)
            .map(|definition| definition.view)
    }

    /// The definition a name names, searching every entered definition.
    pub(crate) fn visible_definition(&self, name: &str) -> Option<View<'a>> {
        self.definitions
            .iter()
            .rev()
            .find(|definition| definition.name == name)
            .map(|definition| definition.view)
    }

    /// Make the definitions a cyclic node declares visible to the nodes below it.
    pub(crate) fn enter(&mut self, view: View<'a>) {
        let Some(declarations) = view.raw.get("$defs").and_then(Value::as_object) else {
            return;
        };
        let visible = self.definitions.len() + declarations.len();
        let entered: Vec<(&'a str, View<'a>)> = declarations
            .iter()
            .map(|(name, declaration)| (name.as_str(), self.document.view(declaration, visible)))
            .collect();
        for (name, view) in entered {
            self.definitions.push(Definition { name, view });
        }
    }
}

/// One schema node as an authored view or a composed stage result.
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
    Array(Rc<Node<'a>>),
    /// Sequence whose positions have their own nodes.
    Tuple(Vec<Rc<Node<'a>>>),
    /// Alternative members.
    Union(Vec<Rc<Node<'a>>>),
    /// Intersected members.
    Intersect(Vec<Rc<Node<'a>>>),
    /// Accepts no value.
    Never,
    /// Accepts every value.
    Any,
    /// Accepts every JSON value, asked as the reference's unknown type.
    Unknown,
}

/// Composed object members in composition order.
pub(crate) struct ObjectNode<'a> {
    /// Member names and their instantiated nodes.
    pub(crate) properties: Vec<(Cow<'a, str>, Rc<Node<'a>>)>,
    /// Authored object the members were instantiated from, when there is one.
    pub(crate) source: Option<View<'a>>,
}

impl<'a> Node<'a> {
    /// The kind this node converts and compares as.
    pub(crate) fn kind(&self) -> Option<SchemaKind> {
        match self {
            Node::Leaf(view) => view.kind.cloned(),
            Node::Constant(_) => Some(SchemaKind::Literal),
            Node::Null => Some(SchemaKind::Null),
            Node::Object(_) => Some(SchemaKind::Object),
            Node::Array(_) => Some(SchemaKind::Array),
            Node::Tuple(_) => Some(SchemaKind::Tuple),
            Node::Union(_) => Some(SchemaKind::Union),
            Node::Intersect(_) => Some(SchemaKind::Intersect),
            Node::Never => Some(SchemaKind::Never),
            Node::Any => Some(SchemaKind::Any),
            Node::Unknown => Some(SchemaKind::Unknown),
        }
    }

    /// The members of an object-like node, object or tuple, in member order.
    pub(crate) fn members(&self) -> Option<Vec<(Cow<'a, str>, Rc<Node<'a>>)>> {
        match self {
            Node::Object(object) => Some(
                object
                    .properties
                    .iter()
                    .map(|(name, node)| (name.clone(), Rc::clone(node)))
                    .collect(),
            ),
            Node::Tuple(items) => Some(
                items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| (Cow::Owned(index.to_string()), Rc::clone(item)))
                    .collect(),
            ),
            _ => None,
        }
    }

    /// The JSON schema this node stands for, borrowed when the node is authored.
    pub(crate) fn schema(&self) -> Cow<'a, Value> {
        match self {
            Node::Leaf(view) => Cow::Borrowed(view.raw),
            Node::Constant(constant) => {
                let mut schema = Map::new();
                schema.insert("const".to_owned(), constant.as_ref().clone());
                Cow::Owned(Value::Object(schema))
            }
            Node::Null => {
                let mut schema = Map::new();
                schema.insert("type".to_owned(), Value::from("null"));
                Cow::Owned(Value::Object(schema))
            }
            Node::Never => Cow::Owned(Value::Bool(false)),
            Node::Any | Node::Unknown => Cow::Owned(Value::Bool(true)),
            _ => Cow::Owned(self.composed_schema()),
        }
    }

    /// Spell a composed node as the schema its members already carry.
    fn composed_schema(&self) -> Value {
        let mut schema = Map::new();
        match self {
            Node::Object(object) => {
                let mut properties = Map::new();
                let mut required = Vec::new();
                for (name, member) in &object.properties {
                    properties.insert(name.as_ref().to_owned(), member.schema().into_owned());
                    required.push(Value::from(name.as_ref()));
                }
                schema.insert("type".to_owned(), Value::from("object"));
                schema.insert("properties".to_owned(), Value::Object(properties));
                schema.insert("required".to_owned(), Value::Array(required));
            }
            Node::Array(item) => {
                schema.insert("type".to_owned(), Value::from("array"));
                schema.insert("items".to_owned(), item.schema().into_owned());
            }
            Node::Tuple(items) => {
                schema.insert("type".to_owned(), Value::from("array"));
                let items = items
                    .iter()
                    .map(|item| item.schema().into_owned())
                    .collect();
                schema.insert("items".to_owned(), Value::Array(items));
            }
            Node::Union(alternatives) => {
                let alternatives = alternatives
                    .iter()
                    .map(|alternative| alternative.schema().into_owned())
                    .collect();
                schema.insert("anyOf".to_owned(), Value::Array(alternatives));
            }
            Node::Intersect(members) => {
                let members = members
                    .iter()
                    .map(|member| member.schema().into_owned())
                    .collect();
                schema.insert("allOf".to_owned(), Value::Array(members));
            }
            Node::Never
            | Node::Any
            | Node::Unknown
            | Node::Leaf(_)
            | Node::Constant(_)
            | Node::Null => {}
        }
        Value::Object(schema)
    }
}
