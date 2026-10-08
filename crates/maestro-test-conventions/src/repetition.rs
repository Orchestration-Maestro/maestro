use ra_ap_rustc_lexer::{FrontmatterAllowed, TokenKind};
use syn::{spanned::Spanned, visit::Visit};

use crate::source::{Member, Source};

/// Reject repeated documentation and adjacent assertion statements.
pub(super) fn check(members: &[Member]) -> Result<(), String> {
    for member in members {
        for source in &member.sources {
            documentation(source)?;
            assertions(source)?;
        }
    }
    Ok(())
}

/// Check documentation tokens without interpreting comment-like text in literals.
fn documentation(source: &Source) -> Result<(), String> {
    let mut offset = ra_ap_rustc_lexer::strip_shebang(&source.contents).unwrap_or(0);
    let mut block = Documentation::default();
    for token in ra_ap_rustc_lexer::tokenize(&source.contents[offset..], FrontmatterAllowed::No) {
        let end = offset + token.len as usize;
        let body = match token.kind {
            TokenKind::LineComment { doc_style: Some(_) } => {
                Some((&source.contents[offset + 3..end], false))
            }
            TokenKind::BlockComment {
                doc_style: Some(_),
                terminated: true,
            } => Some((&source.contents[offset + 3..end - 2], true)),
            TokenKind::Whitespace => None,
            _ => {
                block = Documentation::default();
                None
            }
        };
        if let Some((body, decorated)) = body {
            let start = crate::source::line(&source.contents, offset);
            block.comment(body, start, decorated, source)?;
        }
        offset = end;
    }
    Ok(())
}

#[derive(Default)]
/// Documentation state for a contiguous comment block.
struct Documentation<'a> {
    /// Last nonempty prose line and its source location.
    previous: Option<(&'a str, usize)>,
    /// Opening Markdown code fence, including its marker length.
    fence: Option<&'a str>,
}

impl<'a> Documentation<'a> {
    /// Compare prose lines, removing conventional block-comment decoration.
    fn comment(
        &mut self,
        body: &'a str,
        start: usize,
        decorated: bool,
        source: &Source,
    ) -> Result<(), String> {
        for (index, text) in body.lines().enumerate() {
            let text = text.trim();
            let text = if decorated && index > 0 {
                text.strip_prefix("* ").unwrap_or(text).trim()
            } else {
                text
            };
            if decorated && text == "*" {
                continue;
            }
            self.line(text, start + index, source)?;
        }
        Ok(())
    }

    /// Compare trimmed prose while skipping fenced code and empty lines.
    fn line(&mut self, text: &'a str, line: usize, source: &Source) -> Result<(), String> {
        let text = text.trim();
        if let Some(fence) = self.fence {
            if text.starts_with(fence) && text.bytes().all(|byte| byte == fence.as_bytes()[0]) {
                self.fence = None;
            }
            return Ok(());
        }
        if text.starts_with("```") || text.starts_with("~~~") {
            let marker = text.as_bytes()[0];
            let length = text.bytes().take_while(|byte| *byte == marker).count();
            self.fence = Some(&text[..length]);
            self.previous = None;
            return Ok(());
        }
        if text.is_empty() {
            return Ok(());
        }
        if let Some((previous, first)) = self.previous
            && previous == text
        {
            return Err(format!(
                "{}:{line}: repeated documentation; first at line {first}",
                source.path.display()
            ));
        }
        self.previous = Some((text, line));
        Ok(())
    }
}

/// Check parsed test functions; fragments retain the lexical documentation check.
fn assertions(source: &Source) -> Result<(), String> {
    let mut visitor = Assertions {
        source,
        error: None,
    };
    if let Some(syntax) = &source.syntax {
        visitor.visit_file(syntax);
    }
    visitor.error.map_or(Ok(()), Err)
}

/// Assertion visitor scoped to blocks inside test functions.
struct Assertions<'a> {
    /// File used to attach both locations to diagnostics.
    source: &'a Source,
    /// First repeated assertion, if any.
    error: Option<String>,
}

impl<'ast> Visit<'ast> for Assertions<'_> {
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        if item.attrs.iter().any(|attr| {
            attr.path()
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "test")
        }) {
            self.visit_block(&item.block);
        }
    }

    fn visit_block(&mut self, block: &'ast syn::Block) {
        for pair in block.stmts.windows(2) {
            if let (Some(first), Some(second)) = (assertion(&pair[0]), assertion(&pair[1]))
                && first
                    .path
                    .segments
                    .iter()
                    .map(|segment| &segment.ident)
                    .eq(second.path.segments.iter().map(|segment| &segment.ident))
                && first.path.leading_colon.is_some() == second.path.leading_colon.is_some()
                && std::mem::discriminant(&first.delimiter)
                    == std::mem::discriminant(&second.delimiter)
                && first.tokens.to_string() == second.tokens.to_string()
                && self.error.is_none()
            {
                self.error = Some(format!(
                    "{}:{}: repeated assertion; first at line {}",
                    self.source.path.display(),
                    second.span().start().line,
                    first.span().start().line
                ));
            }
        }
        syn::visit::visit_block(self, block);
    }
}

/// Extract direct assertion statements with transparent expression arguments.
///
/// Calls, method calls, macros, assignments and compound assignments are opaque:
/// repeated tokens may observe changing state, so those remain review judgement.
fn assertion(statement: &syn::Stmt) -> Option<&syn::Macro> {
    let call = match statement {
        syn::Stmt::Macro(statement) => &statement.mac,
        syn::Stmt::Expr(syn::Expr::Macro(expression), _) => &expression.mac,
        _ => return None,
    };
    let name = call.path.segments.last()?.ident.to_string();
    let arguments = call
        .parse_body_with(syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated)
        .ok()?;
    let mut effects = Effects::default();
    for argument in &arguments {
        effects.visit_expr(argument);
    }
    (!effects.opaque
        && (name == "assert"
            || name.starts_with("assert_")
            || name == "debug_assert"
            || name.starts_with("debug_assert_")))
    .then_some(call)
}

#[derive(Default)]
/// Recognize expressions whose evaluation can change state or hides syntax.
struct Effects {
    /// Whether any argument contains opaque or mutating syntax.
    opaque: bool,
}

impl<'ast> Visit<'ast> for Effects {
    fn visit_expr(&mut self, expression: &'ast syn::Expr) {
        self.opaque |= matches!(
            expression,
            syn::Expr::Call(_)
                | syn::Expr::MethodCall(_)
                | syn::Expr::Macro(_)
                | syn::Expr::Assign(_)
        ) || matches!(expression, syn::Expr::Binary(binary) if matches!(binary.op,
                syn::BinOp::AddAssign(_) | syn::BinOp::SubAssign(_) | syn::BinOp::MulAssign(_)
                | syn::BinOp::DivAssign(_) | syn::BinOp::RemAssign(_) | syn::BinOp::BitXorAssign(_)
                | syn::BinOp::BitAndAssign(_) | syn::BinOp::BitOrAssign(_)
                | syn::BinOp::ShlAssign(_) | syn::BinOp::ShrAssign(_)));
        syn::visit::visit_expr(self, expression);
    }
}
