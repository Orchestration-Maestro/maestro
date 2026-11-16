//! Test context inherited during the shared syntax traversal.

use std::ops::Range;

use super::Walker;

impl Walker<'_> {
    /// Visit one subtree under a changed context, then restore the outer context.
    pub(super) fn nested(
        &mut self,
        change: impl FnOnce(&mut Context),
        visit: impl FnOnce(&mut Self),
    ) {
        let outer = self.context;
        change(&mut self.context);
        visit(self);
        self.context = outer;
    }

    /// Record only the outermost environment boundary while retaining its scope for children.
    pub(super) fn environment_gate(
        &mut self,
        span: proc_macro2::Span,
        gated: bool,
        visit: impl FnOnce(&mut Self),
    ) {
        if gated && self.context.environment == Environment::Production {
            self.environment.spans.push(span.byte_range());
        }
        self.nested(
            |context| {
                if gated {
                    context.environment = Environment::Test;
                }
            },
            visit,
        );
    }
}

/// Item kinds by how far the checks inherit their test gate, weakest first.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Gate {
    /// An item inside an `extern` block, which records a test span only.
    Foreign,
    /// An item inside an `impl` or `trait` block, whose syntax is test code.
    Associated,
    /// A module-level item, whose syntax is test code that also exports nothing.
    Item,
}

/// Ownership of environment names at the current traversal position.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Environment {
    /// Names belong to production code unless a boundary says otherwise.
    #[default]
    Production,
    /// A test boundary covers the current syntax.
    Test,
}

/// Test context and traversal position that the check states read.
#[derive(Clone, Copy, Default)]
pub(super) struct Context {
    /// Inside a test-owned environment-name boundary.
    pub(super) environment: Environment,
    /// Strongest test gate around the current syntax; any gate means a recorded test span,
    /// so a gated item nested inside adds no span.
    pub(super) gate: Option<Gate>,
    /// Inside an attribute.
    pub(super) attribute: bool,
    /// Inside a `use` tree.
    pub(super) importing: bool,
    /// Inside the attributes or the name of a method call, which the reference scan skips.
    pub(super) method_parts: bool,
}

/// Convert a syntax span into comparable line and character coordinates.
pub(super) fn span_positions(span: proc_macro2::Span) -> Range<(usize, usize)> {
    let start = span.start();
    let end = span.end();
    (start.line, start.column)..(end.line, end.column)
}
