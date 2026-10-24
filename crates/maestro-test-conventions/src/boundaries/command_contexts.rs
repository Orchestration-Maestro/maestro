//! Single-owner rule for the command-context operations bound to a session.
use syn::{ext::IdentExt, spanned::Spanned, visit::Visit};

use crate::source::{Member, Source, inherit_test_context, test_scope};

/// Crate that declares the operations record.
const DECLARING: &str = "maestro-extensions";
/// Crate that builds the operations for a runtime's current session.
const OWNER: &str = "maestro-app";
/// Record every command-context bind receives.
const ACTIONS: &str = "ExtensionCommandContextActions";

/// Reject a second command-context policy outside the application owner.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let mut diagnostics = Vec::new();
    for member in members {
        for source in &member.sources {
            let Some(syntax) = &source.syntax else {
                continue;
            };
            let mut guard = Guard {
                member: &member.name,
                source,
                testing: test_scope(member, source),
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

/// Located rule violations for one source.
struct Guard<'a> {
    /// Crate the source belongs to.
    member: &'a str,
    /// Source being inspected.
    source: &'a Source,
    /// Whether the current syntax belongs to tests.
    testing: bool,
    /// Located messages in source order.
    found: Vec<(String, (usize, usize), String)>,
}

impl Guard<'_> {
    /// Whether production syntax here may build the operations.
    fn may_build(&self) -> bool {
        self.testing || matches!(self.member, DECLARING | OWNER)
    }

    /// Records a location the operations were built at.
    fn built(&mut self, line: usize, column: usize) {
        self.found.push((
            self.source.path.display().to_string(),
            (line, column),
            format!("the command operations belong to {OWNER}; ask the runtime for its {ACTIONS}"),
        ));
    }
}

impl<'ast> Visit<'ast> for Guard<'_> {
    fn visit_expr_struct(&mut self, built: &'ast syn::ExprStruct) {
        if let Some(segment) = built.path.segments.last()
            && segment.ident.unraw() == ACTIONS
            && !self.may_build()
        {
            let position = segment.ident.span().start();
            self.built(position.line, position.column);
        }
        syn::visit::visit_expr_struct(self, built);
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let syn::Expr::Path(callee) = &*call.func {
            let segments: Vec<String> = callee
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.unraw().to_string())
                .collect();
            let record = segments
                .iter()
                .position(|segment| segment == ACTIONS)
                .is_some_and(|at| at + 1 == segments.len() - 1);
            if record && !self.may_build() {
                let position = call.func.span().start();
                self.built(position.line, position.column);
            }
        }
        syn::visit::visit_expr_call(self, call);
    }

    inherit_test_context!(visit_item, Item, item_attributes);
    inherit_test_context!(visit_impl_item, ImplItem, impl_item_attributes);
    inherit_test_context!(visit_trait_item, TraitItem, trait_item_attributes);
}
