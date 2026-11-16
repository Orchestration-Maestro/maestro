//! Setup children in test code that clear the prepared bundle they inherit.

use std::collections::BTreeSet;

use syn::visit::Visit;

use crate::source::{Member, test_scope};

/// The setup entry a test starts on the development tool.
const SETUP_ENTRY: &str = "nextest-setup";

/// The test-support owner's helper that clears the prepared-bundle inputs.
const CLEARS_PREPARED_BUNDLES: &str = "without_prepared_bundles";

/// Reject a test that starts a setup child without clearing the prepared bundle it inherits.
///
/// A setup child reads `TEST_PREPARE_BASE`, `TEST_PREPARE_OUTPUT` and `TEST_ARCHIVE` from
/// the process that starts it, so a bundle the machine exported for another run changes what
/// the test supplies. The helper lives once in the dedicated test-support owner, and every
/// test-owned source that spells the setup entry in a string literal applies it, directly or
/// through a function of the same file that does. A function is inspected as it is written:
/// macros are not expanded, and an entry named by a constant declared outside a function is
/// not traced to the function that starts it.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let mut diagnostics = Vec::new();
    for member in members {
        let sources = member
            .sources
            .iter()
            .filter(|source| test_scope(member, source));
        for source in sources {
            let Ok(syntax) = syn::parse_file(&source.contents) else {
                continue;
            };
            let mut functions = Functions::default();
            functions.visit_file(&syntax);
            let clearing = functions.clearing();
            diagnostics.extend(
                functions
                    .found
                    .iter()
                    .filter(|function| !clearing.contains(&function.name))
                    .flat_map(|function| function.entries.iter().copied())
                    .map(|line| (source.path.display().to_string(), line)),
            );
        }
    }
    if diagnostics.is_empty() {
        return Ok(());
    }
    diagnostics.sort();
    diagnostics.dedup();
    Err(diagnostics
        .into_iter()
        .map(|(file, line)| {
            format!(
                "{file}:{line}: a setup child inherits the run's prepared bundle; apply maestro_test_process::{CLEARS_PREPARED_BUNDLES} to its command"
            )
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

/// Every function of one source with its setup entries and the names it calls.
#[derive(Default)]
struct Functions {
    /// Functions in the order they end, outermost functions included.
    found: Vec<Function>,
    /// The functions being walked, outermost first.
    open: Vec<Function>,
}

/// One function's setup entries and the functions it calls.
#[derive(Default)]
struct Function {
    /// The declared name.
    name: String,
    /// Lines spelling the setup entry.
    entries: Vec<usize>,
    /// Whether the function applies the helper itself.
    clears: bool,
    /// Names of the functions it calls.
    calls: BTreeSet<String>,
}

impl Functions {
    /// Visit the body of the function named `name` as its own record.
    fn walk(&mut self, name: &syn::Ident, visit: impl FnOnce(&mut Self)) {
        self.open.push(Function {
            name: name.to_string(),
            ..Function::default()
        });
        visit(self);
        if let Some(function) = self.open.pop() {
            self.found.push(function);
        }
    }

    /// The names of the functions that clear the prepared bundle, directly or through a call
    /// to another function of the same source.
    fn clearing(&self) -> BTreeSet<String> {
        let mut clearing: BTreeSet<String> = self
            .found
            .iter()
            .filter(|function| function.clears)
            .map(|function| function.name.clone())
            .collect();
        loop {
            let reached: Vec<String> = self
                .found
                .iter()
                .filter(|function| {
                    !clearing.contains(&function.name)
                        && function.calls.iter().any(|call| clearing.contains(call))
                })
                .map(|function| function.name.clone())
                .collect();
            if reached.is_empty() {
                return clearing;
            }
            clearing.extend(reached);
        }
    }
}

impl<'ast> Visit<'ast> for Functions {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        self.walk(&node.sig.ident, |functions| {
            syn::visit::visit_item_fn(functions, node);
        });
    }

    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        self.walk(&node.sig.ident, |functions| {
            syn::visit::visit_impl_item_fn(functions, node);
        });
    }

    fn visit_trait_item_fn(&mut self, node: &'ast syn::TraitItemFn) {
        self.walk(&node.sig.ident, |functions| {
            syn::visit::visit_trait_item_fn(functions, node);
        });
    }

    fn visit_lit_str(&mut self, node: &'ast syn::LitStr) {
        if node.value() == SETUP_ENTRY
            && let Some(function) = self.open.last_mut()
        {
            function.entries.push(node.span().start().line);
        }
        syn::visit::visit_lit_str(self, node);
    }

    fn visit_path(&mut self, node: &'ast syn::Path) {
        if let Some(function) = self.open.last_mut()
            && let Some(segment) = node.segments.last()
        {
            let name = segment.ident.to_string();
            function.clears |= name == CLEARS_PREPARED_BUNDLES;
            function.calls.insert(name);
        }
        syn::visit::visit_path(self, node);
    }
}
