//! Test-owned temporary directory checks.

use std::collections::BTreeSet;

use syn::{ext::IdentExt, spanned::Spanned, visit::Visit};

use crate::source::{Member, test_scope};

/// Crates whose test directories have moved onto the disposable-directory owner.
///
/// A later migration batch adds the crates it converts; the batch that finishes the
/// migration removes this scope so the check covers every crate.
const MIGRATED_CRATES: &[&str] = &["maestro-daemon", "maestro-session", "maestro-watch"];

/// Directory calls that take ownership of the path they receive.
const OWNERSHIP_CALLS: &[&str] = &[
    "create_dir",
    "create_dir_all",
    "remove_dir",
    "remove_dir_all",
];

/// Method calls that keep a derived path derived.
const DERIVED_METHODS: &[&str] = &[
    "join",
    "parent",
    "to_path_buf",
    "to_owned",
    "clone",
    "unwrap",
    "expect",
    "as_path",
    "canonicalize",
];

/// Reject a temporary directory that migrated-crate test code creates or removes itself.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let mut diagnostics = members
        .iter()
        .filter(|member| MIGRATED_CRATES.contains(&member.name.as_str()))
        .flat_map(member_diagnostics)
        .collect::<Vec<_>>();
    if diagnostics.is_empty() {
        return Ok(());
    }
    diagnostics.sort();
    Err(diagnostics
        .into_iter()
        .map(|(file, line, call)| {
            format!(
                "{file}:{line}: test-owned temporary directory: {call}; use maestro_test_directory::Scratch instead, and add a new crate to TEST_DIRECTORY_CALLERS"
            )
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

/// Report the test sources of one member that own a temporary directory.
fn member_diagnostics(member: &Member) -> Vec<(String, usize, &'static str)> {
    member
        .sources
        .iter()
        .filter(|source| test_scope(member, source))
        .filter_map(|source| {
            let file = source.syntax.as_ref()?;
            let helpers = returned_paths(file);
            Some(
                violations(file, &helpers)
                    .into_iter()
                    .map(|(line, call)| (source.path.display().to_string(), line, call))
                    .collect::<Vec<_>>(),
            )
        })
        .flatten()
        .collect()
}

/// Names of the file's functions that return a path derived from the platform directory.
fn returned_paths(file: &syn::File) -> BTreeSet<String> {
    let mut helpers = BTreeSet::new();
    loop {
        let known = helpers.len();
        for function in functions(file) {
            let name = function.sig.ident.unraw().to_string();
            if !helpers.contains(&name) && returns_derived(&function.block, &helpers) {
                helpers.insert(name);
            }
        }
        if helpers.len() == known {
            return helpers;
        }
    }
}

/// Whether a body returns a path derived from the platform temporary directory.
fn returns_derived(block: &syn::Block, helpers: &BTreeSet<String>) -> bool {
    let locals = derived_locals(&bindings(block), helpers);
    let tail = match block.stmts.last() {
        Some(syn::Stmt::Expr(expr, None)) => derived(expr, &locals, helpers),
        _ => false,
    };
    tail || returned_expressions(block)
        .into_iter()
        .any(|expr| derived(expr, &locals, helpers))
}

/// Directory calls in the file that take a path derived from the platform directory.
fn violations(file: &syn::File, helpers: &BTreeSet<String>) -> Vec<(usize, &'static str)> {
    let mut found = Vec::new();
    for function in functions(file) {
        let locals = derived_locals(&bindings(&function.block), helpers);
        let mut calls = Calls {
            locals: &locals,
            helpers,
            found: Vec::new(),
        };
        calls.visit_block(&function.block);
        found.extend(calls.found);
    }
    found.sort_unstable();
    found.dedup();
    found
}

/// Directory ownership calls found in one function body.
struct Calls<'a> {
    /// Local names bound to a derived path.
    locals: &'a BTreeSet<String>,
    /// Functions whose returned path is derived.
    helpers: &'a BTreeSet<String>,
    /// Located call names.
    found: Vec<(usize, &'static str)>,
}

impl<'ast> Visit<'ast> for Calls<'_> {
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let Some(name) = called(&call.func)
            && let Some(api) = OWNERSHIP_CALLS.iter().copied().find(|api| *api == name)
            && call
                .args
                .iter()
                .any(|arg| derived(arg, self.locals, self.helpers))
        {
            self.found.push((call.span().start().line, api));
        }
        syn::visit::visit_expr_call(self, call);
    }

    /// Skip nested functions, which own their own local bindings.
    fn visit_item_fn(&mut self, _function: &'ast syn::ItemFn) {}
}

/// Whether an expression names a path derived from the platform temporary directory.
fn derived(expr: &syn::Expr, locals: &BTreeSet<String>, helpers: &BTreeSet<String>) -> bool {
    let nested = |inner: &syn::Expr| derived(inner, locals, helpers);
    match expr {
        syn::Expr::Call(call) => {
            called(&call.func).is_some_and(|name| name == "temp_dir" || helpers.contains(&name))
        }
        syn::Expr::MethodCall(call) => {
            let keeps = DERIVED_METHODS.contains(&call.method.unraw().to_string().as_str());
            keeps && nested(&call.receiver)
        }
        syn::Expr::Path(path) => {
            path.qself.is_none()
                && path
                    .path
                    .get_ident()
                    .is_some_and(|ident| locals.contains(&ident.unraw().to_string()))
        }
        syn::Expr::Paren(inner) => nested(&inner.expr),
        syn::Expr::Group(inner) => nested(&inner.expr),
        syn::Expr::Reference(inner) => nested(&inner.expr),
        syn::Expr::Try(inner) => nested(&inner.expr),
        syn::Expr::Cast(inner) => nested(&inner.expr),
        _ => false,
    }
}

/// Local names bound to a path derived from the platform temporary directory.
fn derived_locals(
    bindings: &[(String, &syn::Expr)],
    helpers: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut locals = BTreeSet::new();
    loop {
        let known = locals.len();
        for (name, expr) in bindings {
            if derived(expr, &locals, helpers) {
                locals.insert(name.clone());
            }
        }
        if locals.len() == known {
            return locals;
        }
    }
}

/// Last segment of a called path.
fn called(func: &syn::Expr) -> Option<String> {
    match func {
        syn::Expr::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.unraw().to_string()),
        _ => None,
    }
}

/// Local bindings with their initializer expressions, excluding nested functions.
fn bindings(block: &syn::Block) -> Vec<(String, &syn::Expr)> {
    /// Collected local bindings with their initializer expressions.
    struct Lets<'a>(Vec<(String, &'a syn::Expr)>);

    impl<'ast> Visit<'ast> for Lets<'ast> {
        fn visit_local(&mut self, local: &'ast syn::Local) {
            if let syn::Pat::Ident(pattern) = &local.pat
                && let Some(init) = &local.init
            {
                self.0.push((pattern.ident.unraw().to_string(), &init.expr));
            }
            syn::visit::visit_local(self, local);
        }

        fn visit_item_fn(&mut self, _function: &'ast syn::ItemFn) {}
    }

    let mut lets = Lets(Vec::new());
    lets.visit_block(block);
    lets.0
}

/// Function items in the file, including those declared inside modules and blocks.
fn functions(file: &syn::File) -> Vec<&syn::ItemFn> {
    /// Collected function items.
    struct Functions<'a>(Vec<&'a syn::ItemFn>);

    impl<'ast> Visit<'ast> for Functions<'ast> {
        fn visit_item_fn(&mut self, function: &'ast syn::ItemFn) {
            self.0.push(function);
            syn::visit::visit_item_fn(self, function);
        }
    }

    let mut functions = Functions(Vec::new());
    functions.visit_file(file);
    functions.0
}

/// Explicit return expressions of one body.
fn returned_expressions(block: &syn::Block) -> Vec<&syn::Expr> {
    /// Collected return expressions.
    struct Returns<'a>(Vec<&'a syn::Expr>);

    impl<'ast> Visit<'ast> for Returns<'ast> {
        fn visit_expr_return(&mut self, expr: &'ast syn::ExprReturn) {
            if let Some(value) = &expr.expr {
                self.0.push(value);
            }
            syn::visit::visit_expr_return(self, expr);
        }

        fn visit_item_fn(&mut self, _function: &'ast syn::ItemFn) {}
    }

    let mut returns = Returns(Vec::new());
    returns.visit_block(block);
    returns.0
}
