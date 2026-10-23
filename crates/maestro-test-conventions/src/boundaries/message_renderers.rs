//! Single-owner rule for retained message renderer calls.
use std::collections::BTreeMap;

use syn::{ext::IdentExt, visit::Visit};

use crate::source::{Member, Source, cfg_test, inherit_test_context, test_layout};

/// Crate that owns the projected call and the callbacks it retains.
const OWNER: &str = "maestro-extensions";
/// Frontend that may only use the projected call.
const FRONTEND: &str = "maestro-chat";
/// Record that binds a callback to its registering event bus.
const PROJECTED: &str = "MessageRendererCall";
/// Raw extension machinery the frontend must not reach directly.
const RAW: &[&str] = &[
    "MessageRenderer",
    "CallbackEmitter",
    "EventBus",
    "get_message_renderer",
    "invoke_callback",
];

/// Reject raw renderer plumbing in frontend production syntax and foreign declarations.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let mut diagnostics = Vec::new();
    for member in members {
        for source in &member.sources {
            let Some(syntax) = &source.syntax else {
                continue;
            };
            let mut aliases = Aliases::default();
            aliases.visit_file(syntax);
            let mut guard = Guard {
                member: &member.name,
                source,
                testing: member.name.starts_with("maestro-test-")
                    || test_layout(&source.path, &member.directory)
                    || cfg_test(&syntax.attrs),
                aliases: aliases.0,
                found: Vec::new(),
            };
            guard.visit_file(syntax);
            diagnostics.extend(guard.found);
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        diagnostics.sort_by(|left, right| (&left.0, left.1).cmp(&(&right.0, right.1)));
        Err(diagnostics
            .into_iter()
            .map(|(file, (line, _), message)| format!("{file}:{line}: {message}"))
            .collect::<Vec<_>>()
            .join("\n"))
    }
}

/// Known raw aliases collected across one parsed file.
#[derive(Default)]
struct Aliases(BTreeMap<String, String>);

/// Known import candidates from a use tree.
fn imports(tree: &syn::UseTree, aliases: &mut BTreeMap<String, String>) {
    match tree {
        syn::UseTree::Path(path) => imports(&path.tree, aliases),
        syn::UseTree::Name(name) => {
            let raw = name.ident.unraw().to_string();
            if RAW.contains(&raw.as_str()) {
                aliases.insert(raw.clone(), raw);
            }
        }
        syn::UseTree::Rename(name) => {
            let raw = name.ident.unraw().to_string();
            if RAW.contains(&raw.as_str()) {
                aliases.insert(name.rename.unraw().to_string(), raw);
            }
        }
        syn::UseTree::Group(group) => {
            for tree in &group.items {
                imports(tree, aliases);
            }
        }
        syn::UseTree::Glob(_) => {}
    }
}

impl<'ast> Visit<'ast> for Aliases {
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        imports(&item.tree, &mut self.0);
    }
}

/// Located rule violations for one source.
struct Guard<'a> {
    /// Crate the source belongs to.
    member: &'a str,
    /// Source being inspected.
    source: &'a Source,
    /// Whether the current syntax belongs to tests.
    testing: bool,
    /// Raw identifiers imported under another local name.
    aliases: BTreeMap<String, String>,
    /// Located messages in source order.
    found: Vec<(String, (usize, usize), String)>,
}

impl Guard<'_> {
    /// Records a raw identifier reached from the frontend.
    fn raw(&mut self, written: &str, span: proc_macro2::Span) {
        if self.member != FRONTEND || self.testing {
            return;
        }
        let Some(raw) = self
            .aliases
            .get(written)
            .cloned()
            .or_else(|| RAW.contains(&written).then(|| written.to_owned()))
        else {
            return;
        };
        let position = span.start();
        self.found.push((
            self.source.path.display().to_string(),
            (position.line, position.column),
            format!("raw {raw} outside {OWNER}; bind a renderer through {PROJECTED} and call that"),
        ));
    }

    /// Records a foreign declaration of the projected record.
    fn declaration(&mut self, name: &syn::Ident) {
        if name.unraw() != PROJECTED {
            return;
        }
        if self.member == OWNER && !self.source.path.ends_with("types/renderers.rs") {
            let position = name.span().start();
            self.found.push((
                self.source.path.display().to_string(),
                (position.line, position.column),
                format!("duplicate {PROJECTED} declaration; {OWNER} declares it once in types/renderers.rs"),
            ));
            return;
        }
        if self.member != OWNER {
            let position = name.span().start();
            self.found.push((
                self.source.path.display().to_string(),
                (position.line, position.column),
                format!(
                    "{PROJECTED} declaration belongs to {OWNER}, not {}",
                    self.member
                ),
            ));
        }
    }
}

impl<'ast> Visit<'ast> for Guard<'_> {
    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        let name = call.method.unraw().to_string();
        self.raw(&name, call.method.span());
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_path(&mut self, path: &'ast syn::Path) {
        if let Some(last) = path.segments.last() {
            self.raw(&last.ident.unraw().to_string(), last.ident.span());
        }
        syn::visit::visit_path(self, path);
    }

    fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
        if !self.testing {
            self.declaration(&item.ident);
        }
        syn::visit::visit_item_struct(self, item);
    }

    fn visit_item_enum(&mut self, item: &'ast syn::ItemEnum) {
        if !self.testing {
            self.declaration(&item.ident);
        }
        syn::visit::visit_item_enum(self, item);
    }

    fn visit_item_type(&mut self, item: &'ast syn::ItemType) {
        if !self.testing {
            self.declaration(&item.ident);
        }
        syn::visit::visit_item_type(self, item);
    }

    inherit_test_context!(visit_item, Item, item_attributes);
    inherit_test_context!(visit_impl_item, ImplItem, impl_item_attributes);
    inherit_test_context!(visit_trait_item, TraitItem, trait_item_attributes);
}
