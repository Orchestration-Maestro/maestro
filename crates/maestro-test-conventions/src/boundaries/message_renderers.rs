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

/// Relative path of the file that declares the projected record.
const OWNER_FILE: &str = "src/types/renderers.rs";

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
                owner_file: source
                    .path
                    .strip_prefix(&member.directory)
                    .is_ok_and(|relative| relative == std::path::Path::new(OWNER_FILE)),
                declarations: 0,
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
        diagnostics.dedup();
        Err(diagnostics
            .into_iter()
            .map(|(file, (line, _), message)| format!("{file}:{line}: {message}"))
            .collect::<Vec<_>>()
            .join("\n"))
    }
}

/// One item a use tree imports.
struct Import {
    /// Path segments as written, ending with the imported item.
    path: Vec<syn::Ident>,
    /// Local name the import binds, absent for a glob.
    binding: Option<syn::Ident>,
}

impl Import {
    /// Name of the item the import binds, where `self` names the module above it.
    fn item(&self) -> Option<String> {
        let (last, above) = self.path.split_last()?;
        let name = if last == "self" { above.last()? } else { last };
        Some(name.unraw().to_string())
    }
}

/// Every item one use tree imports, whatever nesting or aliasing it writes.
fn flatten(tree: &syn::UseTree) -> Vec<Import> {
    let mut imports = Vec::new();
    collect(tree, &[], &mut imports);
    imports
}

/// Appends the items of one use tree below the segments already written.
fn collect(tree: &syn::UseTree, above: &[syn::Ident], imports: &mut Vec<Import>) {
    match tree {
        syn::UseTree::Path(path) => {
            let mut prefix = above.to_vec();
            prefix.push(path.ident.clone());
            collect(&path.tree, &prefix, imports);
        }
        syn::UseTree::Name(name) => imports.push(Import {
            path: beneath(above, &name.ident),
            binding: Some(name.ident.clone()),
        }),
        syn::UseTree::Rename(rename) => imports.push(Import {
            path: beneath(above, &rename.ident),
            binding: Some(rename.rename.clone()),
        }),
        syn::UseTree::Group(group) => {
            for tree in &group.items {
                collect(tree, above, imports);
            }
        }
        syn::UseTree::Glob(_) => imports.push(Import {
            path: above.to_vec(),
            binding: None,
        }),
    }
}

/// The segments of the path that names one imported item.
fn beneath(above: &[syn::Ident], item: &syn::Ident) -> Vec<syn::Ident> {
    let mut path = above.to_vec();
    path.push(item.clone());
    path
}

/// Known raw aliases collected across one parsed file.
#[derive(Default)]
struct Aliases(BTreeMap<String, String>);

impl<'ast> Visit<'ast> for Aliases {
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        for import in flatten(&item.tree) {
            let Some(binding) = &import.binding else {
                continue;
            };
            if let Some(name) = import.item().filter(|name| RAW.contains(&name.as_str())) {
                self.0.insert(binding.unraw().to_string(), name);
            }
        }
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
    /// Whether this source is the file that must hold the single declaration.
    owner_file: bool,
    /// Production declarations of the projected record seen in this source.
    declarations: usize,
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

    /// Records every restricted name a use tree writes, prefix segments included.
    fn imported(&mut self, tree: &syn::UseTree) {
        for import in flatten(tree) {
            for segment in &import.path {
                self.raw(&segment.unraw().to_string(), segment.span());
            }
        }
    }

    /// Records a foreign declaration of the projected record.
    fn declaration(&mut self, name: &syn::Ident) {
        if name.unraw() != PROJECTED {
            return;
        }
        let position = name.span().start();
        if self.member != OWNER {
            self.found.push((
                self.source.path.display().to_string(),
                (position.line, position.column),
                format!(
                    "{PROJECTED} declaration belongs to {OWNER}, not {}",
                    self.member
                ),
            ));
            return;
        }
        self.declarations += 1;
        let message = if !self.owner_file {
            format!("duplicate {PROJECTED} declaration; {OWNER} declares it once in {OWNER_FILE}")
        } else if self.declarations > 1 {
            format!("second {PROJECTED} declaration; {OWNER} declares it once in {OWNER_FILE}")
        } else {
            return;
        };
        self.found.push((
            self.source.path.display().to_string(),
            (position.line, position.column),
            message,
        ));
    }
}

impl<'ast> Visit<'ast> for Guard<'_> {
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        self.imported(&item.tree);
        syn::visit::visit_item_use(self, item);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        let name = call.method.unraw().to_string();
        self.raw(&name, call.method.span());
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_path(&mut self, path: &'ast syn::Path) {
        for segment in &path.segments {
            self.raw(&segment.ident.unraw().to_string(), segment.ident.span());
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
