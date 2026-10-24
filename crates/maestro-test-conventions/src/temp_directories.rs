//! Test-owned temporary directory checks.

use syn::visit::Visit;

use crate::source::{Member, Source, inherit_test_context, test_scope};

/// Crates whose test directories have moved onto the disposable-directory owner.
///
/// A later migration batch adds the crates it converts; the batch that finishes the
/// migration removes this scope so the check covers every crate.
const MIGRATED_CRATES: &[&str] = &["maestro-daemon", "maestro-session", "maestro-watch"];

/// Spelling that names the platform temporary directory.
const PLATFORM_DIRECTORY: &str = "temp_dir";

/// Reject a platform temporary directory that migrated-crate test code names itself.
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
        .map(|(file, line)| {
            format!(
                "{file}:{line}: test-owned code names the platform temporary directory; use maestro_test_directory::Scratch or maestro_test_directory::temporary_root, and add a new crate to TEST_DIRECTORY_CALLERS"
            )
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

/// Report the located platform-directory references of one member.
fn member_diagnostics(member: &Member) -> Vec<(String, usize)> {
    member
        .sources
        .iter()
        .flat_map(|source| {
            references(member, source)
                .into_iter()
                .map(|line| (source.path.display().to_string(), line))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Lines of one source that name the platform temporary directory in test-owned code,
/// or import it anywhere.
fn references(member: &Member, source: &Source) -> Vec<usize> {
    let Some(file) = source.syntax.as_ref() else {
        return Vec::new();
    };
    let mut visitor = References {
        testing: test_scope(member, source),
        importing: false,
        lines: Vec::new(),
    };
    visitor.visit_file(file);
    visitor.lines.sort_unstable();
    visitor.lines.dedup();
    visitor.lines
}

/// Platform-directory references located in one file.
struct References {
    /// Whether the current item inherits test ownership.
    testing: bool,
    /// Whether the current position is a `use` import, whose alias also counts.
    importing: bool,
    /// Located reference lines.
    lines: Vec<usize>,
}

impl References {
    /// Record one identifier that names the platform temporary directory.
    fn record(&mut self, ident: &proc_macro2::Ident) {
        if (self.testing || self.importing) && ident == PLATFORM_DIRECTORY {
            self.lines.push(ident.span().start().line);
        }
    }
}

impl<'ast> Visit<'ast> for References {
    fn visit_ident(&mut self, ident: &'ast proc_macro2::Ident) {
        self.record(ident);
    }

    fn visit_use_tree(&mut self, tree: &'ast syn::UseTree) {
        let importing = self.importing;
        self.importing = true;
        syn::visit::visit_use_tree(self, tree);
        self.importing = importing;
    }

    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        if self.testing || self.importing {
            token_references(&invocation.tokens, &mut self.lines);
        }
        syn::visit::visit_macro(self, invocation);
    }

    inherit_test_context!(visit_item, Item, item_attributes);
    inherit_test_context!(visit_impl_item, ImplItem, impl_item_attributes);
    inherit_test_context!(visit_trait_item, TraitItem, trait_item_attributes);
}

/// Record platform-directory identifiers anywhere inside macro tokens, nesting included.
fn token_references(tokens: &proc_macro2::TokenStream, lines: &mut Vec<usize>) {
    for tree in tokens.clone() {
        match tree {
            proc_macro2::TokenTree::Ident(ident) if ident == PLATFORM_DIRECTORY => {
                lines.push(ident.span().start().line);
            }
            proc_macro2::TokenTree::Group(group) => token_references(&group.stream(), lines),
            _ => {}
        }
    }
}
