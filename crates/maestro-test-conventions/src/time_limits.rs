use std::collections::{BTreeMap, BTreeSet};

use syn::{
    ext::IdentExt, parse::Parser, parse::discouraged::Speculative, spanned::Spanned, visit::Visit,
};

use crate::source::{Member, cfg_test, inherit_test_context, test_layout};

/// Reject recognized optional polling macros and timer calls in test-owned parsed sources.
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
                match api {
                    Diagnostic::Timer(api) => format!("{file}:{line}: test-owned time limit: {api}; wait for completion instead"),
                    Diagnostic::OptionalPoll => format!("{file}:{line}: optional futures_util::poll! in test source; pin the future and call std::future::Future::poll once with std::task::Context::from_waker(std::task::Waker::noop())"),
                }
            })
            .collect::<Vec<_>>()
            .join("\n"))
    }
}

/// Recognized test-source portability or synchronization violation.
enum Diagnostic {
    /// Canonical timer identity.
    Timer(String),
    /// Single polling requires an optional dependency feature.
    OptionalPoll,
}

/// Test ownership and located policy occurrences for one source.
struct Calls {
    /// Imported local names and their known authored identities.
    imports: BTreeMap<String, BTreeSet<String>>,
    /// Whether the current syntax belongs to tests.
    testing: bool,
    /// Canonical labels with source coordinates.
    found: Vec<((usize, usize), Diagnostic)>,
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
            for api in TIMERS.iter().chain(OPTIONAL_POLL) {
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

/// Retain aliases of known timer functions, optional macros or their namespaces.
fn bind(names: &mut BTreeMap<String, BTreeSet<String>>, name: &syn::Ident, path: String) {
    if matches!(
        path.as_str(),
        "tokio" | "tokio::time" | "std" | "std::thread" | "futures_util"
    ) || TIMERS.contains(&path.as_str())
        || OPTIONAL_POLL.contains(&path.as_str())
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

/// Canonical optional single-poll macro identity.
const OPTIONAL_POLL: &[&str] = &["futures_util::poll"];

/// Resolve a self import to its containing namespace.
fn import_path(prefix: &str, name: &syn::Ident) -> String {
    if name == "self" {
        prefix.trim_end_matches("::").into()
    } else {
        format!("{prefix}{}", name.unraw())
    }
}

/// Known API and namespace aliases collected across a parsed file.
#[derive(Default)]
struct Aliases(BTreeMap<String, BTreeSet<String>>);

impl<'ast> Visit<'ast> for Aliases {
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        imports(&item.tree, "", &mut self.0);
    }
}

impl Calls {
    /// Prefer direct identities, then match every known alias candidate.
    fn resolve<'a>(&self, name: &str, known: &[&'a str]) -> Option<&'a str> {
        if let Some(api) = known.iter().copied().find(|api| *api == name) {
            return Some(api);
        }
        let (first, suffix) = name.split_once("::").unwrap_or((name, ""));
        let candidates = self.imports.get(first)?;
        known.iter().copied().find(|api| {
            candidates.iter().any(|candidate| {
                (suffix.is_empty() && *api == candidate)
                    || api
                        .strip_prefix(candidate.as_str())
                        .and_then(|rest| rest.strip_prefix("::"))
                        == Some(suffix)
            })
        })
    }

    /// Record an optional macro at its authored invocation coordinates.
    fn optional_poll(&mut self, path: &syn::Path) {
        if self.resolve(&path_name(path), OPTIONAL_POLL).is_some() {
            let position = path.span().start();
            self.found
                .push(((position.line, position.column), Diagnostic::OptionalPoll));
        }
    }

    /// Inspect literal invocation tokens without expanding their outer macro.
    fn nested_macros(&mut self, tokens: proc_macro2::TokenStream) {
        let parser = |input: syn::parse::ParseStream<'_>| self.scan_macro_tokens(input);
        let _ = parser.parse2(tokens);
    }

    /// Consume every token tree, descending into groups and literal invocations.
    fn scan_macro_tokens(&mut self, input: syn::parse::ParseStream<'_>) -> syn::Result<()> {
        while !input.is_empty() {
            let candidate = input.fork();
            let Ok(path) = candidate.call(syn::Path::parse_mod_style) else {
                self.nested_group(input.parse()?);
                continue;
            };
            input.advance_to(&candidate);
            if candidate.parse::<syn::Token![!]>().is_ok()
                && let Ok(group) = candidate.parse::<proc_macro2::Group>()
            {
                self.optional_poll(&path);
                self.nested_macros(group.stream());
                input.advance_to(&candidate);
            }
        }
        Ok(())
    }

    /// Descend into grouped tokens while leaving literals opaque.
    fn nested_group(&mut self, token: proc_macro2::TokenTree) {
        if let proc_macro2::TokenTree::Group(group) = token {
            self.nested_macros(group.stream());
        }
    }
}

impl<'ast> Visit<'ast> for Calls {
    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        if self.testing {
            self.optional_poll(&invocation.path);
            self.nested_macros(invocation.tokens.clone());
        }
        syn::visit::visit_macro(self, invocation);
    }

    inherit_test_context!(visit_item, Item, item_attributes);
    inherit_test_context!(visit_impl_item, ImplItem, impl_item_attributes);
    inherit_test_context!(visit_trait_item, TraitItem, trait_item_attributes);
    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        let name = call.method.unraw().to_string();
        if self.testing && timed_wait(&name) {
            let position = call.span().start();
            self.found
                .push(((position.line, position.column), Diagnostic::Timer(name)));
        }
        syn::visit::visit_expr_method_call(self, call);
    }
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        let mut callee = &*call.func;
        while let syn::Expr::Paren(parenthesized) = callee {
            callee = &parenthesized.expr;
        }
        if let syn::Expr::Path(path) = callee {
            let name = path_name(&path.path);
            let last = name.rsplit("::").next().unwrap_or(&name);
            let timer = if timed_wait(last) {
                Some(last)
            } else {
                self.resolve(&name, TIMERS)
            };
            if self.testing
                && let Some(api) = timer
            {
                let position = call.span().start();
                self.found.push((
                    (position.line, position.column),
                    Diagnostic::Timer(api.into()),
                ));
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

/// Normalize raw identifiers without changing authored source coordinates.
fn path_name(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|part| part.ident.unraw().to_string())
        .collect::<Vec<_>>()
        .join("::")
}
