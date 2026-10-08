use std::path::Path;

use pulldown_cmark::{Event, Parser, Tag};
use ra_ap_rustc_lexer::{FrontmatterAllowed, TokenKind};
use syn::{spanned::Spanned, visit::Visit};

use crate::source::{Member, Source};

/// Reject repeated documentation and adjacent assertion statements.
pub(super) fn check(members: &[Member]) -> Result<(), String> {
    for member in members {
        for source in &member.sources {
            let generated_bindings = member.name == "maestro-extensions-wasm"
                && source.path == member.directory.join("src/bindings.rs");
            if !generated_bindings {
                documentation(source)?;
            }
            assertions(source, &member.directory)?;
        }
    }
    Ok(())
}

/// Check documentation tokens without interpreting comment-like text in literals.
fn documentation(source: &Source) -> Result<(), String> {
    let mut offset = ra_ap_rustc_lexer::strip_shebang(&source.contents).unwrap_or(0);
    let mut block = Documentation::default();
    let mut style = None;
    for token in ra_ap_rustc_lexer::tokenize(&source.contents[offset..], FrontmatterAllowed::No) {
        let end = offset + token.len as usize;
        let body = match token.kind {
            TokenKind::LineComment {
                doc_style: Some(owner),
            } => Some((&source.contents[offset + 3..end], owner)),
            TokenKind::BlockComment {
                doc_style: Some(_), ..
            } => {
                let line = crate::source::line(&source.contents, offset);
                return Err(format!(
                    "{}:{line}: block documentation: use /// or //!",
                    source.path.display()
                ));
            }
            TokenKind::Whitespace => None,
            _ => {
                block.check(source)?;
                block = Documentation::default();
                style = None;
                None
            }
        };
        if let Some((body, owner)) = body {
            if style != Some(owner) {
                block.check(source)?;
                block = Documentation::default();
                style = Some(owner);
            }
            let start = crate::source::line(&source.contents, offset);
            block.comment(body, start);
        }
        offset = end;
    }
    block.check(source)
}

#[derive(Default)]
/// Documentation state for a contiguous comment block.
struct Documentation {
    /// Markdown source retaining indentation for code-block parsing.
    markdown: String,
    /// Physical source locations of the Markdown lines.
    lines: Vec<usize>,
}

impl Documentation {
    /// Retain a documentation line, removing one separator space.
    fn comment(&mut self, body: &str, start: usize) {
        self.markdown
            .push_str(body.strip_prefix(' ').unwrap_or(body));
        self.markdown.push('\n');
        self.lines.push(start);
    }

    /// Compare physical lines, excluding parser-selected code block ranges.
    fn check(&self, source: &Source) -> Result<(), String> {
        let code: Vec<_> = Parser::new(&self.markdown)
            .into_offset_iter()
            .filter_map(|(event, range)| {
                matches!(event, Event::Start(Tag::CodeBlock(_))).then_some(range)
            })
            .collect();
        let mut previous = None;
        let mut offset = 0;
        for (index, text) in self.markdown.lines().enumerate() {
            let end = offset + text.len();
            let inside_code = code
                .iter()
                .any(|range| range.start < end && offset < range.end);
            offset = end + 1;
            if inside_code {
                previous = None;
                continue;
            }
            let text = text.trim();
            if text.is_empty() {
                continue;
            }
            let line = self.lines[index];
            if let Some((prior, first)) = previous
                && prior == text
            {
                return Err(format!(
                    "{}:{line}: repeated documentation; first at line {first}",
                    source.path.display()
                ));
            }
            previous = Some((text, line));
        }
        Ok(())
    }
}

/// Check local module declarations and assertions selected by file or inline context.
fn assertions(source: &Source, directory: &Path) -> Result<(), String> {
    let relative = source.path.strip_prefix(directory).ok();
    let test_layout = source
        .path
        .file_name()
        .is_some_and(|name| name == "tests.rs")
        || relative.is_some_and(|path| {
            path.parent().is_some_and(|parent| {
                parent
                    .components()
                    .any(|component| component.as_os_str() == "tests")
            })
        });
    let configured = source
        .syntax
        .as_ref()
        .is_some_and(|file| cfg_test(&file.attrs));
    let mut visitor = Assertions {
        source,
        in_src: source.path.starts_with(directory.join("src")),
        error: None,
        test_module: test_layout || configured,
        test_function: false,
    };
    if let Some(syntax) = &source.syntax {
        visitor.visit_file(syntax);
    }
    visitor.error.map_or(Ok(()), Err)?;
    if visitor.in_src && configured && !test_layout {
        return Err(format!(
            "{}:1: test file convention: use tests.rs or a tests directory for #![cfg(test)]",
            source.path.display()
        ));
    }
    Ok(())
}

/// Recognize the direct test-only configuration shared by files and modules.
fn cfg_test(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("cfg")
            && attr
                .parse_args::<syn::Path>()
                .is_ok_and(|path| path.is_ident("test"))
    })
}

/// Assertion visitor scoped to blocks inside test functions.
struct Assertions<'a> {
    /// File used to attach both locations to diagnostics.
    source: &'a Source,
    /// Whether source-local test module declarations must follow the convention.
    in_src: bool,
    /// First convention or repetition violation, if any.
    error: Option<String>,
    /// Whether the containing module is selected for tests.
    test_module: bool,
    /// Whether traversal is inside a selected function.
    test_function: bool,
}

impl<'ast> Visit<'ast> for Assertions<'_> {
    fn visit_expr_const(&mut self, _expression: &'ast syn::ExprConst) {}

    fn visit_item(&mut self, item: &'ast syn::Item) {
        let active = self.test_function;
        self.test_function = false;
        match item {
            syn::Item::Fn(function) => self.visit_item_fn(function),
            syn::Item::Mod(module) => self.visit_item_mod(module),
            syn::Item::Impl(item) => syn::visit::visit_item_impl(self, item),
            syn::Item::Trait(item) => syn::visit::visit_item_trait(self, item),
            _ => {}
        }
        self.test_function = active;
    }

    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        let prior = self.test_module;
        let configured = cfg_test(&item.attrs);
        let named = item.ident == "tests";
        if self.in_src
            && (configured != named
                || ((configured || named)
                    && item.attrs.iter().any(|attr| attr.path().is_ident("path"))))
            && self.error.is_none()
        {
            self.error = Some(format!(
                "{}:{}: test module convention: use #[cfg(test)] mod tests without #[path]",
                self.source.path.display(),
                item.ident.span().start().line,
            ));
        }
        self.test_module |= configured;
        syn::visit::visit_item_mod(self, item);
        self.test_module = prior;
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        self.function(&item.attrs, &item.block);
    }

    fn visit_impl_item(&mut self, item: &'ast syn::ImplItem) {
        if let syn::ImplItem::Fn(function) = item {
            self.function(&function.attrs, &function.block);
        }
    }

    fn visit_trait_item(&mut self, item: &'ast syn::TraitItem) {
        if let syn::TraitItem::Fn(function) = item
            && let Some(block) = &function.default
        {
            self.function(&function.attrs, block);
        }
    }

    fn visit_block(&mut self, block: &'ast syn::Block) {
        if !self.test_function {
            return;
        }
        for pair in block.stmts.windows(2) {
            if let (Some((first, first_attrs)), Some((second, second_attrs))) =
                (assertion(&pair[0]), assertion(&pair[1]))
                && first_attrs == second_attrs
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

impl Assertions<'_> {
    /// Enter only functions selected by their test attribute or containing module.
    fn function(&mut self, attrs: &[syn::Attribute], block: &syn::Block) {
        let prior = self.test_function;
        self.test_function = self.test_module
            || attrs.iter().any(|attr| {
                attr.path()
                    .segments
                    .last()
                    .is_some_and(|segment| segment.ident == "test")
            });
        self.visit_block(block);
        self.test_function = prior;
    }
}

/// Extract direct assertion statements with transparent expression arguments.
///
/// Calls, method calls, awaiting, macros and all assignments are opaque:
/// repeated tokens may observe changing state, so those remain review judgement.
fn assertion(statement: &syn::Stmt) -> Option<(&syn::Macro, &[syn::Attribute])> {
    let (call, attrs) = match statement {
        syn::Stmt::Macro(statement) => (&statement.mac, statement.attrs.as_slice()),
        syn::Stmt::Expr(syn::Expr::Macro(expression), _) => {
            (&expression.mac, expression.attrs.as_slice())
        }
        _ => return None,
    };
    let name = call.path.segments.last()?.ident.to_string();
    let mut effects = Effects::default();
    if name == "assert_matches" || name == "debug_assert_matches" {
        call.parse_body_with(|input: syn::parse::ParseStream<'_>| {
            effects.matching_arguments(input)
        })
        .ok()?;
    } else {
        let arguments = call
            .parse_body_with(
                syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated,
            )
            .ok()?;
        for argument in &arguments {
            effects.visit_expr(argument);
        }
    }
    (!effects.opaque
        && (name == "assert"
            || name.starts_with("assert_")
            || name == "debug_assert"
            || name.starts_with("debug_assert_")))
    .then_some((call, attrs))
}

#[derive(Default)]
/// Recognize expressions whose evaluation can change state or hides syntax.
struct Effects {
    /// Whether any argument contains opaque or mutating syntax.
    opaque: bool,
}

impl Effects {
    /// Inspect the value, pattern, optional guard and message of a matching assertion.
    fn matching_arguments(&mut self, input: syn::parse::ParseStream<'_>) -> syn::Result<()> {
        self.visit_expr(&input.parse()?);
        input.parse::<syn::Token![,]>()?;
        self.visit_pat(&syn::Pat::parse_multi_with_leading_vert(input)?);
        if input.peek(syn::Token![if]) {
            input.parse::<syn::Token![if]>()?;
            self.visit_expr(&input.parse()?);
        }
        if input.peek(syn::Token![,]) {
            input.parse::<syn::Token![,]>()?;
            for argument in
                syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated(input)?
            {
                self.visit_expr(&argument);
            }
        }
        Ok(())
    }
}

impl<'ast> Visit<'ast> for Effects {
    fn visit_pat(&mut self, pattern: &'ast syn::Pat) {
        self.opaque |= matches!(pattern, syn::Pat::Macro(_));
        syn::visit::visit_pat(self, pattern);
    }

    fn visit_expr(&mut self, expression: &'ast syn::Expr) {
        self.opaque |= matches!(
            expression,
            syn::Expr::Call(_)
                | syn::Expr::Await(_)
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
