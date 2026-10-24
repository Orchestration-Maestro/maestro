//! Single-owner rule for the shared helpers of the shipped extension examples.

use syn::spanned::Spanned;
use syn::visit::{self, Visit};

use crate::source::{Member, Source};

/// Crate whose example tests use the shared helpers.
const GUEST: &str = "maestro-extensions-wasm";
/// Example-test directory inside the guest crate.
const TESTS: &str = "src/tests";
/// File name suffix of one example's tests.
const SUFFIX: &str = "_example.rs";
/// Operation that decodes a document member.
const DECODED: &str = "example_support::decoded";
/// Operation that releases every registration and returns its names.
const RELEASED: &str = "example_support::release_all";

/// Reject example tests that write a shared example operation inline.
///
/// The shared decoding, registration-release walk and released-name comparison
/// live once in the example support module; a per-example test that repeats one
/// of them, or discards the names it releases, is reported with its file and line.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let mut diagnostics = Vec::new();
    for member in members.iter().filter(|member| member.name == GUEST) {
        let sources = member
            .sources
            .iter()
            .filter(|source| example_test(member, source));
        for source in sources {
            let Some(syntax) = &source.syntax else {
                continue;
            };
            let mut guard = Guard::default();
            guard.visit_file(syntax);
            let file = source.path.display().to_string();
            diagnostics.extend(
                guard
                    .found
                    .into_iter()
                    .map(|(position, message)| (file.clone(), position, message)),
            );
        }
    }
    if diagnostics.is_empty() {
        return Ok(());
    }
    diagnostics.sort_by(|left, right| (&left.0, left.1).cmp(&(&right.0, right.1)));
    diagnostics.dedup();
    Err(diagnostics
        .into_iter()
        .map(|(file, (line, _), message)| format!("{file}:{line}: {message}"))
        .collect::<Vec<_>>()
        .join("\n"))
}

/// Whether `source` is one example's test file in the guest crate.
fn example_test(member: &Member, source: &Source) -> bool {
    source
        .path
        .strip_prefix(&member.directory)
        .is_ok_and(|relative| {
            relative.starts_with(TESTS)
                && relative
                    .file_name()
                    .and_then(std::ffi::OsStr::to_str)
                    .is_some_and(|name| name.ends_with(SUFFIX))
        })
}

/// Located inline copies in one parsed example test.
#[derive(Default)]
struct Guard {
    /// One-based position and message per copy.
    found: Vec<((usize, usize), String)>,
}

impl<'ast> Visit<'ast> for Guard {
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if named(&call.func, "from_str") && decodes_member(call) {
            self.found.push((
                position(&call.func),
                format!("inline member decode; use {DECODED}"),
            ));
        }
        visit::visit_expr_call(self, call);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if call.method == "next_to_release" {
            self.found.push((
                position(&call.method),
                format!("inline registration release walk; use {RELEASED}"),
            ));
        }
        visit::visit_expr_method_call(self, call);
    }

    fn visit_stmt(&mut self, statement: &'ast syn::Stmt) {
        if let syn::Stmt::Expr(syn::Expr::Call(call), Some(_)) = statement
            && named(&call.func, "release_all")
        {
            self.found.push((
                position(&call.func),
                format!("compare the names {RELEASED} returns with the expected registrations"),
            ));
        }
        visit::visit_stmt(self, statement);
    }
}

/// Whether `function` names `name` in its last path segment.
fn named(function: &syn::Expr, name: &str) -> bool {
    matches!(
        function,
        syn::Expr::Path(path)
            if path.path.segments.last().is_some_and(|segment| segment.ident == name)
    )
}

/// Whether the call reads one text member of an indexed document.
fn decodes_member(call: &syn::ExprCall) -> bool {
    let mut access = MemberAccess::default();
    for argument in &call.args {
        access.visit_expr(argument);
    }
    access.indexed && access.text
}

/// The accesses an inline member decode combines.
#[derive(Default)]
struct MemberAccess {
    /// Whether an index expression was read.
    indexed: bool,
    /// Whether indexed text was taken as a string.
    text: bool,
}

impl Visit<'_> for MemberAccess {
    fn visit_expr_index(&mut self, index: &syn::ExprIndex) {
        self.indexed = true;
        visit::visit_expr_index(self, index);
    }

    fn visit_expr_method_call(&mut self, call: &syn::ExprMethodCall) {
        if call.method == "as_str" {
            self.text = true;
        }
        visit::visit_expr_method_call(self, call);
    }
}

/// One-based line and column where `spanned` starts.
fn position(spanned: &impl Spanned) -> (usize, usize) {
    let start = spanned.span().start();
    (start.line, start.column)
}
