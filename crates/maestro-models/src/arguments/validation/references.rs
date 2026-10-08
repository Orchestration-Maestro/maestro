//! Offline schema resource resolution over borrowed locations with live anchor bindings.

use std::{collections::BTreeMap, rc::Rc};

use percent_encoding::percent_decode_str;
use serde_json::Value;
use url::Url;

use super::{
    check::{Batch, Instruction, Job, Mode, is_schema},
    diagnostics,
};

/// Base used to resolve identifiers of a document without an absolute root.
const DEFAULT_BASE: &str = "http://unknown/";

#[derive(Clone)]
/// Schema resource whose identifier is already resolved against its lexical parent.
struct Resource<'a> {
    /// Original schema declaring the resource, or the document root.
    schema: &'a Value,
    /// Resolved identifier, absent when the declared identifier cannot be joined.
    uri: Option<Rc<Url>>,
}

#[derive(Clone)]
/// Original schema node together with the lexical resource chain that contains it.
pub(super) struct Location<'a> {
    /// Original, unmodified schema node.
    pub schema: &'a Value,
    /// Resources enclosing the current one, outermost first.
    ancestors: Rc<[Resource<'a>]>,
    /// Innermost resource containing the node.
    resource: Resource<'a>,
}

impl<'a> Location<'a> {
    /// Start at the document root, which is an implicit resource.
    pub fn root(schema: &'a Value) -> Self {
        let base = Url::parse(DEFAULT_BASE).ok();
        let uri = match identifier(schema) {
            Some(id) => base.and_then(|base| base.join(id).ok()),
            None => base,
        };
        Self {
            schema,
            ancestors: Rc::default(),
            resource: Resource {
                schema,
                uri: uri.map(Rc::new),
            },
        }
    }

    /// Descend to a nested node, entering a new resource when it declares an identifier.
    pub fn child(&self, schema: &'a Value) -> Self {
        let Some(id) = identifier(schema) else {
            return Self {
                schema,
                ..self.clone()
            };
        };
        Self {
            schema,
            ancestors: self.chain().cloned().collect(),
            resource: Resource {
                schema,
                uri: self
                    .resource
                    .uri
                    .as_ref()
                    .and_then(|uri| uri.join(id).ok())
                    .map(Rc::new),
            },
        }
    }

    /// Compare node identities; the lexical chain follows from the node.
    fn same(&self, other: &Self) -> bool {
        std::ptr::eq(self.schema, other.schema)
    }

    /// Whether the node is the root of the resource containing it.
    fn is_resource_root(&self) -> bool {
        std::ptr::eq(self.schema, self.resource.schema)
    }

    /// Enclosing resources and the current one, outermost first.
    fn chain(&self) -> impl Iterator<Item = &Resource<'a>> {
        self.ancestors.iter().chain([&self.resource])
    }

    /// Location of the current resource's own root.
    fn resource_root(&self) -> Self {
        Self {
            schema: self.resource.schema,
            ..self.clone()
        }
    }

    /// Location of each resource in the chain, outermost first.
    fn resources(&self) -> impl Iterator<Item = Self> {
        self.chain().enumerate().map(|(depth, resource)| Self {
            schema: resource.schema,
            ancestors: self.ancestors.iter().take(depth).cloned().collect(),
            resource: resource.clone(),
        })
    }
}

#[derive(Clone, Default)]
/// Live recursive and dynamic anchor bindings; the outermost binding wins.
struct Scope<'a> {
    /// First entered schema declaring a true recursive anchor, with or without an identifier.
    recursive: Option<Location<'a>>,
    /// First active location declaring each dynamic anchor name.
    dynamic: BTreeMap<&'a str, Location<'a>>,
}

impl<'a> Scope<'a> {
    /// Bind the entered schema's own `$recursiveAnchor: true` and `$dynamicAnchor`, keeping outer bindings.
    fn bind_entry(&mut self, entered: &Location<'a>) {
        if entered.schema.get("$recursiveAnchor") == Some(&Value::Bool(true))
            && self.recursive.is_none()
        {
            self.recursive = Some(entered.clone());
        }
        if let Some(name) = entered.schema.get("$dynamicAnchor").and_then(Value::as_str) {
            self.dynamic.entry(name).or_insert_with(|| entered.clone());
        }
    }

    /// Bind the dynamic anchors below a resource root without crossing nested identifiers.
    fn bind_resource(&mut self, resource: &Location<'a>) {
        let mut pending: Vec<_> = children(resource.schema).into_iter().rev().collect();
        while let Some(schema) = pending.pop() {
            if identifier(schema).is_some() {
                continue;
            }
            if let Some(name) = schema.get("$dynamicAnchor").and_then(Value::as_str) {
                self.dynamic.entry(name).or_insert_with(|| Location {
                    schema,
                    ..resource.clone()
                });
            }
            pending.extend(children(schema).into_iter().rev());
        }
    }

    /// Whether entering the location can bind anything.
    fn binds(entered: &Location<'a>) -> bool {
        identifier(entered.schema).is_some()
            || ["$recursiveAnchor", "$dynamicAnchor"]
                .iter()
                .any(|keyword| entered.schema.get(keyword).is_some())
    }

    /// Enter the location: a schema with an identifier binds the dynamic anchors below it first.
    fn enter(&mut self, entered: &Location<'a>) {
        if identifier(entered.schema).is_some() {
            self.bind_resource(entered);
        }
        self.bind_entry(entered);
    }

    /// Compare effective bindings by node identity.
    fn same(&self, other: &Self) -> bool {
        let recursive = match (&self.recursive, &other.recursive) {
            (Some(left), Some(right)) => left.same(right),
            (left, right) => left.is_none() && right.is_none(),
        };
        recursive
            && self.dynamic.len() == other.dynamic.len()
            && self.dynamic.iter().all(|(name, binding)| {
                other
                    .dynamic
                    .get(name)
                    .is_some_and(|other| binding.same(other))
            })
    }
}

#[derive(Clone)]
/// Schema location paired with the anchor bindings live while evaluating it.
pub(super) struct Context<'a> {
    /// Schema node being applied and its lexical resources.
    pub location: Location<'a>,
    /// Bindings shared with every context that has not added its own.
    scope: Rc<Scope<'a>>,
}

impl<'a> Context<'a> {
    /// Begin at the document root with its own anchors live.
    pub fn root(location: Location<'a>) -> Self {
        let mut scope = Scope::default();
        scope.enter(&location);
        Self {
            location,
            scope: Rc::new(scope),
        }
    }

    /// Descend lexically, entering the child so its own anchors become live.
    pub fn child(&self, schema: &'a Value) -> Self {
        let location = self.location.child(schema);
        let mut scope = Rc::clone(&self.scope);
        if Scope::binds(&location) {
            Rc::make_mut(&mut scope).enter(&location);
        }
        Self { location, scope }
    }

    /// Follow a reference, keeping live bindings, entering the target's resources not yet
    /// entered and then the target itself.
    pub fn follow(&self, target: Location<'a>) -> Self {
        let mut scope = Rc::clone(&self.scope);
        for resource in target.resources() {
            let entered = self
                .location
                .chain()
                .any(|entered| std::ptr::eq(entered.schema, resource.schema));
            if !entered {
                Rc::make_mut(&mut scope).enter(&resource);
            }
        }
        if !target.is_resource_root() && Scope::binds(&target) {
            Rc::make_mut(&mut scope).enter(&target);
        }
        Self {
            location: target,
            scope,
        }
    }

    /// Compare schema nodes and effective bindings for non-progress cycle detection.
    pub fn same(&self, other: &Self) -> bool {
        self.location.same(&other.location) && self.scope.same(&other.scope)
    }
}

#[derive(Clone, Copy)]
/// Reference keyword whose target selection differs.
enum ReferenceKind {
    /// Static reference resolved against the enclosing identifier.
    Ref,
    /// Reference searched in its own resource or, when that resource declares a recursive
    /// anchor, in the subtree of the first entered schema declaring one.
    Recursive,
    /// Static reference replaced by a live dynamic binding.
    Dynamic,
}

/// Targets of the reference keywords the context's schema applies, in application order;
/// `None` marks a reference that offline resolution cannot satisfy.
pub(super) fn targets<'a>(root: &Location<'a>, context: &Context<'a>) -> Vec<Option<Location<'a>>> {
    [
        ("$ref", ReferenceKind::Ref),
        ("$recursiveRef", ReferenceKind::Recursive),
        ("$dynamicRef", ReferenceKind::Dynamic),
    ]
    .into_iter()
    .filter_map(|(keyword, kind)| {
        let reference = context.location.schema.get(keyword)?.as_str()?;
        Some(target(root, context, kind, reference).filter(|target| is_schema(target.schema)))
    })
    .collect()
}

/// Schedule all reference keywords through the shared offline resolver.
pub(super) fn instructions<'a>(root: &Location<'a>, job: &Job<'a>) -> Vec<Instruction<'a>> {
    /// Rejecting target schema used when offline resolution fails.
    const FALSE: Value = Value::Bool(false);
    targets(root, &job.context)
        .into_iter()
        .map(|target| {
            let resolved =
                target.map_or_else(|| job.same_instance(&FALSE), |target| job.resolved(target));
            Instruction::Children(Batch::new(Mode::All, [resolved]))
        })
        .collect()
}

/// Resolve a reference from the context's resource, applying the keyword's search rule.
fn target<'a>(
    root: &Location<'a>,
    context: &Context<'a>,
    kind: ReferenceKind,
    reference: &str,
) -> Option<Location<'a>> {
    let resource = &context.location.resource;
    let joined = |base: &Resource<'a>| base.uri.as_ref()?.join(reference).ok();
    let target = match kind {
        ReferenceKind::Recursive
            if resource.schema.get("$recursiveAnchor") == Some(&Value::Bool(true)) =>
        {
            let search = context.scope.recursive.as_ref()?;
            resolve(search, &joined(&search.resource)?)
        }
        ReferenceKind::Ref | ReferenceKind::Dynamic if !reference.starts_with('#') => {
            resolve(root, &joined(resource)?)
        }
        _ => resolve(&context.location.resource_root(), &joined(resource)?),
    }?;
    Some(match kind {
        ReferenceKind::Dynamic => overridden(&context.scope, target, reference),
        _ => target,
    })
}

/// Replace a dynamic-anchor target by the outermost live binding of that name, except for pointers.
fn overridden<'a>(scope: &Scope<'a>, target: Location<'a>, reference: &str) -> Location<'a> {
    let pointer = reference.split('#').nth(1).is_some_and(|fragment| {
        percent_decode_str(fragment)
            .decode_utf8_lossy()
            .starts_with('/')
    });
    target
        .schema
        .get("$dynamicAnchor")
        .and_then(Value::as_str)
        .filter(|_| !pointer)
        .and_then(|name| scope.dynamic.get(name))
        .map_or(target, Clone::clone)
}

/// Identifier string declared by a schema, which starts a resource.
fn identifier(schema: &Value) -> Option<&str> {
    schema.get("$id").and_then(Value::as_str)
}

/// Child nodes of an object or array in canonical traversal order.
fn children(schema: &Value) -> Vec<&Value> {
    match schema {
        Value::Object(object) => diagnostics::entries(object)
            .into_iter()
            .map(|(_, child)| child)
            .collect(),
        Value::Array(array) => array.iter().collect(),
        _ => Vec::new(),
    }
}

/// Search identifiers, pointers and anchors below a location, keeping the last match.
fn resolve<'a>(search: &Location<'a>, target: &Url) -> Option<Location<'a>> {
    let identity = target.as_str().split('#').next()?;
    let fragment = percent_decode_str(target.fragment().unwrap_or(""))
        .decode_utf8()
        .ok()?;
    let mut pending = vec![search.clone()];
    let mut result = None;
    while let Some(node) = pending.pop() {
        if let Some(found) = matched(search, &node, target, identity, &fragment) {
            result = Some(found);
        } else {
            pending.extend(
                children(node.schema)
                    .into_iter()
                    .rev()
                    .map(|child| node.child(child)),
            );
        }
    }
    result
}

/// Match a node in the target's resource by identifier alias, pointer or anchor name.
///
/// The search start counts as a root even when it is not a resource root, so a recursive binding on
/// a schema without an identifier is its own target for the empty fragment and the base of pointers.
fn matched<'a>(
    search: &Location<'a>,
    node: &Location<'a>,
    target: &Url,
    identity: &str,
    fragment: &str,
) -> Option<Location<'a>> {
    let uri = node
        .resource
        .uri
        .as_ref()
        .filter(|uri| uri.as_str().split('#').next() == Some(identity))?;
    let is_root = node.is_resource_root() || node.same(search);
    let alias = identifier(node.schema).is_some_and(|id| id.starts_with('#'));
    if (alias && uri.as_ref() == target) || (fragment.is_empty() && is_root) {
        return Some(node.clone());
    }
    if fragment.starts_with('/')
        && is_root
        && let Some(found) = pointer(node, fragment)
    {
        return Some(found);
    }
    ["$anchor", "$dynamicAnchor"]
        .iter()
        .any(|key| node.schema.get(key).and_then(Value::as_str) == Some(fragment))
        .then(|| node.clone())
}

/// Traverse decoded pointer segments, entering each crossed resource once.
fn pointer<'a>(start: &Location<'a>, fragment: &str) -> Option<Location<'a>> {
    let mut location = start.clone();
    for segment in fragment.strip_prefix('/')?.split('/') {
        let key = segment.replace("~1", "/").replace("~0", "~");
        if matches!(key.as_str(), "__proto__" | "constructor" | "prototype") {
            return None;
        }
        let next = match location.schema {
            Value::Object(object) => object.get(&key)?,
            Value::Array(array) => {
                if !key.bytes().all(|byte| byte.is_ascii_digit())
                    || (key != "0" && key.starts_with('0'))
                {
                    return None;
                }
                array.get(key.parse::<usize>().ok()?)?
            }
            _ => return None,
        };
        location = location.child(next);
    }
    Some(location)
}
