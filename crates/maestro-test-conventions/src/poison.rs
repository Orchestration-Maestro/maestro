//! Production ownership of poison-tolerant shared-state acquisition.

use syn::spanned::Spanned;

use crate::graph::policy;
use crate::source::Member;

/// Whether the member's production sources are checked for their own recovery helper.
///
/// The owner holds the one implementation, so its own functions are the operations the
/// check sends callers to.
fn checked(member: &Member) -> bool {
    member.name != policy::LOCK
}

/// Reject a production helper that recovers shared state on its own.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let mut diagnostics = members
        .iter()
        .filter(|member| checked(member))
        .flat_map(|member| {
            member.sources.iter().flat_map(move |source| {
                source
                    .walk
                    .iter()
                    .flat_map(|walk| &walk.poison)
                    .map(move |line| (source.path.display().to_string(), *line))
            })
        })
        .collect::<Vec<_>>();
    if diagnostics.is_empty() {
        return Ok(());
    }
    diagnostics.sort();
    Err(diagnostics
        .into_iter()
        .map(|(file, line)| {
            format!(
                "{file}:{line}: production code defines its own poison-tolerant lock acquisition; call maestro_lock::lock, maestro_lock::read or maestro_lock::write"
            )
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

/// Definitions of an independent recovery helper in one production source.
#[derive(Default)]
pub(crate) struct Definitions {
    /// Located lines, in source order.
    lines: Vec<usize>,
}

impl Definitions {
    /// Record a function whose whole body is the acquisition.
    pub(crate) fn body(&mut self, block: &syn::Block) {
        if let Some(line) = recovered_body(block) {
            self.lines.push(line);
        }
    }

    /// Record a closure whose whole body is the acquisition.
    pub(crate) fn closure(&mut self, closure: &syn::ExprClosure) {
        let recovered = match &*closure.body {
            syn::Expr::Block(block) => recovered_body(&block.block),
            body => recovering(body).then(|| body.span().line()),
        };
        if let Some(line) = recovered {
            self.lines.push(line);
        }
    }

    /// Record the acquisition shape anywhere inside a macro's tokens.
    pub(crate) fn macro_tokens(&mut self, invocation: &syn::Macro) {
        if let Some(line) = token_recovery(&invocation.tokens) {
            self.lines.push(line);
        }
    }

    /// The located lines in order, each once.
    pub(crate) fn finish(mut self) -> Vec<usize> {
        self.lines.sort_unstable();
        self.lines.dedup();
        self.lines
    }
}

/// Recognize a body that only acquires a lock, recovering a poetically poisoned value.
fn recovered_body(block: &syn::Block) -> Option<usize> {
    let [syn::Stmt::Expr(expression, None)] = block.stmts.as_slice() else {
        return None;
    };
    recovering(expression).then(|| expression.span().line())
}

/// Recognize `<lock>.lock()`, `<lock>.read()` or `<lock>.write()` followed by recovery.
fn recovering(expression: &syn::Expr) -> bool {
    match expression {
        syn::Expr::MethodCall(call) => {
            call.method == "unwrap_or_else"
                && acquired(&call.receiver)
                && call.args.iter().any(names_poison)
        }
        syn::Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Deref(_)) => {
            recovering(&unary.expr)
        }
        syn::Expr::Paren(paren) => recovering(&paren.expr),
        _ => false,
    }
}

/// Recognize a zero-argument acquisition of a retained lock.
fn acquired(expression: &syn::Expr) -> bool {
    let syn::Expr::MethodCall(call) = expression else {
        return false;
    };
    call.args.is_empty() && matches!(call.method.to_string().as_str(), "lock" | "read" | "write")
}

/// Recognize an argument that names the poison recovery.
fn names_poison(expression: &syn::Expr) -> bool {
    match expression {
        syn::Expr::Path(path) => path
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "into_inner"),
        syn::Expr::Reference(reference) => names_poison(&reference.expr),
        syn::Expr::Paren(paren) => names_poison(&paren.expr),
        _ => false,
    }
}

/// Recognize a generated helper body inside a macro definition.
///
/// A macro's tokens parse as no function, so the check reads the single-expression
/// block a generated helper's body is written as. Statements of a larger body, and
/// arguments of another macro call, are not helper bodies.
fn token_recovery(tokens: &proc_macro2::TokenStream) -> Option<usize> {
    for tree in tokens.clone() {
        let proc_macro2::TokenTree::Group(group) = tree else {
            continue;
        };
        if group.delimiter() != proc_macro2::Delimiter::Brace {
            continue;
        }
        if let Some(line) = token_recovery(&group.stream()) {
            return Some(line);
        }
        let trees: Vec<_> = group.stream().into_iter().collect();
        let statement = trees.iter().any(
            |tree| matches!(tree, proc_macro2::TokenTree::Punct(punct) if punct.as_char() == ';'),
        );
        if !statement && let Some(line) = shape_recovery(&trees) {
            return Some(line);
        }
    }
    None
}

/// Recognize the acquisition shape in one token sequence.
fn shape_recovery(trees: &[proc_macro2::TokenTree]) -> Option<usize> {
    for (index, tree) in trees.iter().enumerate() {
        let proc_macro2::TokenTree::Ident(acquiring) = tree else {
            continue;
        };
        if !matches!(acquiring.to_string().as_str(), "lock" | "read" | "write") {
            continue;
        }
        let following = |offset: usize| trees.get(index + offset);
        let called = matches!(following(1), Some(proc_macro2::TokenTree::Group(group)) if group.stream().is_empty());
        let recovered = matches!(following(2), Some(proc_macro2::TokenTree::Punct(punct)) if punct.as_char() == '.')
            && matches!(following(3), Some(proc_macro2::TokenTree::Ident(name)) if name == "unwrap_or_else");
        if called && recovered {
            return Some(acquiring.span().start().line);
        }
    }
    None
}

/// Report the located line of one syntax span.
trait SpanLine {
    /// One-based line where this span starts.
    fn line(&self) -> usize;
}

impl SpanLine for proc_macro2::Span {
    fn line(&self) -> usize {
        self.start().line
    }
}
