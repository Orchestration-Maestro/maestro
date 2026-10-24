//! Single-owner rule for temporary files and external-editor launches in the chat frontend.
use std::path::Path;

use syn::visit::Visit;

use crate::source::{Member, inherit_test_context, test_scope, visit_token_identifiers};

/// Frontend whose callers delegate these effects.
const FRONTEND: &str = "maestro-chat";

/// Files that create temporary files or launch the editor only through the owners.
const CALLERS: &[&str] = &[
    "src/components/extension_editor.rs",
    "src/interactive_mode/actions.rs",
];

/// Names that create or write files or launch a process themselves.
const EFFECTS: &[&str] = &[
    "OpenOptions",
    "File",
    "write",
    "write_all",
    "spawn_sync",
    "windows_shell_launch",
];

/// Reject file creation, writes and editor launches that a caller performs itself.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let mut diagnostics = Vec::new();
    for member in members.iter().filter(|member| member.name == FRONTEND) {
        for source in &member.sources {
            let caller = source
                .path
                .strip_prefix(&member.directory)
                .is_ok_and(|relative| CALLERS.iter().any(|caller| relative == Path::new(caller)));
            let Some(syntax) = source.syntax.as_ref().filter(|_| caller) else {
                continue;
            };
            let mut effects = Effects {
                testing: test_scope(member, source),
                lines: Vec::new(),
            };
            effects.visit_file(syntax);
            diagnostics.extend(effects.lines.into_iter().map(|(line, name)| {
                format!(
                    "{}:{line}: `{name}` creates, writes or launches outside its owner; use temporary_file::write_temporary_file or external_editor::edit_external_file",
                    source.path.display()
                )
            }));
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics.join("\n"))
    }
}

/// Effect names located in production syntax of one caller.
struct Effects {
    /// Whether the current item inherits test ownership.
    testing: bool,
    /// Located effect names with their lines.
    lines: Vec<(usize, String)>,
}

impl Effects {
    /// Record one identifier that names an effect.
    fn record(&mut self, ident: &proc_macro2::Ident) {
        if !self.testing && EFFECTS.iter().any(|effect| ident == effect) {
            self.lines
                .push((ident.span().start().line, ident.to_string()));
        }
    }
}

impl<'ast> Visit<'ast> for Effects {
    fn visit_ident(&mut self, ident: &'ast proc_macro2::Ident) {
        self.record(ident);
    }

    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        visit_token_identifiers(&invocation.tokens, &mut |ident| self.record(ident));
        syn::visit::visit_macro(self, invocation);
    }

    inherit_test_context!(visit_item, Item, item_attributes);
    inherit_test_context!(visit_impl_item, ImplItem, impl_item_attributes);
    inherit_test_context!(visit_trait_item, TraitItem, trait_item_attributes);
}

#[cfg(test)]
mod tests;
