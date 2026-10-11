use std::collections::{BTreeMap, BTreeSet};

use syn::{ext::IdentExt, spanned::Spanned, visit::Visit};

use crate::source::{
    Member, cfg_test, impl_item_attributes, item_attributes, test_layout, trait_item_attributes,
};

/// Reject recognized timer calls in test-owned parsed sources.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let mut diagnostics = Vec::new();
    for member in members {
        for source in &member.sources {
            let Some(syntax) = &source.syntax else {
                continue;
            };
            let mut aliases = Aliases::default();
            aliases.visit_file(syntax);
            let mut calls = Calls {
                testing: member.name.starts_with("maestro-test-")
                    || test_layout(&source.path, &member.directory)
                    || cfg_test(&syntax.attrs),
                found: Vec::new(),
                imports: aliases.0,
            };
            calls.visit_file(syntax);
            diagnostics.extend(
                calls
                    .found
                    .into_iter()
                    .map(|(position, api)| (source.path.display().to_string(), position, api)),
            );
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        diagnostics.sort_by(|left, right| (&left.0, left.1).cmp(&(&right.0, right.1)));
        Err(diagnostics
            .into_iter()
            .map(|(file, (line, _), api)| {
                format!("{file}:{line}: test-owned time limit: {api}; wait for completion instead")
            })
            .collect::<Vec<_>>()
            .join("\n"))
    }
}

/// Test ownership and located timer occurrences for one source.
struct Calls {
    /// Imported local names and their known authored identities.
    imports: BTreeMap<String, BTreeSet<String>>,
    /// Whether the current syntax belongs to tests.
    testing: bool,
    /// Canonical labels with source coordinates.
    found: Vec<((usize, usize), String)>,
}

/// Collect known import candidates from a use tree.
fn imports(tree: &syn::UseTree, prefix: &str, names: &mut BTreeMap<String, BTreeSet<String>>) {
    match tree {
        syn::UseTree::Path(path) => imports(
            &path.tree,
            &format!("{prefix}{}::", path.ident.unraw()),
            names,
        ),
        syn::UseTree::Name(name) => bind(names, &name.ident, import_path(prefix, &name.ident)),
        syn::UseTree::Rename(name) => bind(names, &name.rename, import_path(prefix, &name.ident)),
        syn::UseTree::Group(group) => {
            for tree in &group.items {
                imports(tree, prefix, names);
            }
        }
        syn::UseTree::Glob(_) => {
            for api in TIMERS {
                if api
                    .rsplit_once("::")
                    .is_some_and(|(module, _)| module == prefix.trim_end_matches("::"))
                {
                    names
                        .entry(api.rsplit("::").next().unwrap_or(api).into())
                        .or_default()
                        .insert((*api).into());
                }
            }
        }
    }
}

/// Retain only aliases of the known timer functions or their namespaces.
fn bind(names: &mut BTreeMap<String, BTreeSet<String>>, name: &syn::Ident, path: String) {
    if matches!(
        path.as_str(),
        "tokio" | "tokio::time" | "std" | "std::thread"
    ) || TIMERS.contains(&path.as_str())
    {
        names
            .entry(name.unraw().to_string())
            .or_default()
            .insert(path);
    }
}

/// Canonical free-function timer identities.
const TIMERS: &[&str] = &[
    "tokio::time::timeout",
    "tokio::time::timeout_at",
    "std::thread::park_timeout",
];

/// Resolve a self import to its containing namespace.
fn import_path(prefix: &str, name: &syn::Ident) -> String {
    if name == "self" {
        prefix.trim_end_matches("::").into()
    } else {
        format!("{prefix}{}", name.unraw())
    }
}

/// Timer namespace aliases collected across a parsed file.
#[derive(Default)]
struct Aliases(BTreeMap<String, BTreeSet<String>>);

impl<'ast> Visit<'ast> for Aliases {
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        imports(&item.tree, "", &mut self.0);
    }
}

impl Calls {
    /// Prefer direct timer identities, then match every known alias candidate.
    fn timer<'a>(&self, name: &'a str) -> Option<&'a str> {
        let last = name.rsplit("::").next().unwrap_or(name);
        if TIMERS.contains(&name) {
            return Some(name);
        }
        if timed_wait(last) {
            return Some(last);
        }
        let (first, suffix) = name.split_once("::").unwrap_or((name, ""));
        let candidates = self.imports.get(first)?;
        TIMERS.iter().copied().find(|api| {
            candidates.iter().any(|candidate| {
                (suffix.is_empty() && *api == candidate)
                    || api
                        .strip_prefix(candidate.as_str())
                        .and_then(|rest| rest.strip_prefix("::"))
                        == Some(suffix)
            })
        })
    }
}

impl<'ast> Visit<'ast> for Calls {
    fn visit_item(&mut self, item: &'ast syn::Item) {
        let testing = self.testing;
        self.testing |= cfg_test(item_attributes(item));
        syn::visit::visit_item(self, item);
        self.testing = testing;
    }
    fn visit_impl_item(&mut self, item: &'ast syn::ImplItem) {
        let testing = self.testing;
        self.testing |= cfg_test(impl_item_attributes(item));
        syn::visit::visit_impl_item(self, item);
        self.testing = testing;
    }
    fn visit_trait_item(&mut self, item: &'ast syn::TraitItem) {
        let testing = self.testing;
        self.testing |= cfg_test(trait_item_attributes(item));
        syn::visit::visit_trait_item(self, item);
        self.testing = testing;
    }
    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        let name = call.method.unraw().to_string();
        if self.testing && timed_wait(&name) {
            let position = call.span().start();
            self.found.push(((position.line, position.column), name));
        }
        syn::visit::visit_expr_method_call(self, call);
    }
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        let mut callee = &*call.func;
        while let syn::Expr::Paren(parenthesized) = callee {
            callee = &parenthesized.expr;
        }
        if let syn::Expr::Path(path) = callee {
            let name = path
                .path
                .segments
                .iter()
                .map(|part| part.ident.unraw().to_string())
                .collect::<Vec<_>>()
                .join("::");
            if self.testing
                && let Some(name) = self.timer(&name)
            {
                let position = call.span().start();
                self.found
                    .push(((position.line, position.column), name.into()));
            }
        }
        syn::visit::visit_expr_call(self, call);
    }
}

/// Recognize the syntactically forbidden timed method names.
fn timed_wait(name: &str) -> bool {
    matches!(
        name,
        "recv_timeout" | "recv_deadline" | "wait_timeout" | "wait_timeout_while"
    )
}
