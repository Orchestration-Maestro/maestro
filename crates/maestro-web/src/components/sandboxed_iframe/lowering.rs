//! Compile supplied content for native browser execution with Rust-owned captures.
//!
//! Program returns become breaks to a private enclosing label. The fall-through
//! assignment stays inside that label, so a finally that cancels a return also
//! discards its value. Each finalizer restores its saved value when it completes
//! normally, preserving an outer return if an inner return is cancelled locally.
//! Native control flow retains iterator cleanup precedence.
use std::{collections::HashSet, fmt, fmt::Write as _};

use oxc_allocator::Allocator;
use oxc_ast::{
    AstKind,
    ast::{
        ArrowFunctionBody, Class, Expression, FunctionBody, Program, ReturnStatement, Statement,
        TryStatement,
    },
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_span::{GetSpan as _, SourceType, Span};

/// Supplied content compiled for a native capture accessor.
///
/// Capture properties use the reserved `__maestro` prefix with a decimal suffix.
/// The installer restores an existing descriptor before entering a program body
/// or invoking a captured function, refusing a protected property instead of overwriting it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compiled {
    /// Browser-executable source; modules are used for programs, classic scripts for providers.
    code: String,
    /// Temporary global accessor installed before executing the source.
    capture: String,
}

impl Compiled {
    /// Browser-executable source, before HTML carrier encoding or policy hashing.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Global accessor restored before program-body entry or captured-function invocation.
    #[must_use]
    pub fn capture(&self) -> &str {
        &self.capture
    }
}

/// Why supplied content could not be compiled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The supplied body or expression is not valid JavaScript.
    Syntax(String),
    /// A parsed source range or body could not be represented.
    SourceRange,
}

impl fmt::Display for Error {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(message) => write!(output, "invalid supplied content: {message}"),
            Self::SourceRange => output.write_str("cannot represent supplied source range"),
        }
    }
}
impl std::error::Error for Error {}

/// Compile an asynchronous body into a native inline module.
///
/// A native getter removes the carrier and returns a private outcome observer.
/// Its `value` setter receives `[true, value]` or `[false, error]`. Rust assimilates
/// returned thenables before completion callbacks. No function is dynamically compiled.
///
/// # Errors
/// Returns parser diagnostics for malformed asynchronous bodies, including static imports
/// and exports. The browser remains the final gate for semantic early errors.
///
/// # Examples
/// ```
/// use maestro_web::components::sandboxed_iframe::lowering;
/// let compiled = lowering::program("await null; return 42;")?;
/// assert!(!compiled.capture().is_empty());
/// # Ok::<(), lowering::Error>(())
/// ```
pub fn program(source: &str) -> Result<Compiled, Error> {
    let input = format!("async()=>{{\n{source}\n}}");
    let allocator = Allocator::default();
    let parsed = parse(&allocator, &input, SourceType::mjs())?;
    let body = function_body(&parsed)?;
    let mut names = Names::default();
    names.visit_program(&parsed);
    let (capture, value, exit, error, observer) = (
        names.fresh(),
        names.fresh(),
        names.fresh(),
        names.fresh(),
        names.fresh(),
    );
    let mut completions = Completions {
        value: &value,
        exit: &exit,
        source: &input,
        names: &mut names,
        edits: Vec::new(),
        error: None,
    };
    completions.visit_statements(&body.statements);
    if let Some(error) = completions.error {
        return Err(error);
    }
    let hoisted = hoist(&body.statements, &input, &mut completions.edits)?;
    let span = Span::new(body.span.start + 1, body.span.end - 1);
    let content = replace(&input, span, completions.edits)?;
    let code = format!(
        "const {observer}={capture};\nlet {value};\ntry{{{exit}:{{\n{hoisted}{content}\n{value}=void 0;\n}}{observer}.value=[true,{value}];}}catch({error}){{{observer}.value=[false,{error}];}}"
    );
    Ok(Compiled { code, capture })
}

/// Compile a supplied provider expression for synchronous capture and invocation from Rust.
///
/// The capture receives the expression's completion: `[true, value]` when it yields a
/// value and `[false, error]` when it throws, so a failure stops installation without the
/// browser's error reporting. An array element never infers a name for an anonymous
/// function. The supplied expression retains classic-script scope and strictness.
///
/// # Errors
/// Rejects invalid expressions or content outside the supplied expression.
pub fn provider(source: &str) -> Result<Compiled, Error> {
    let input = format!("(\n{source}\n)");
    let allocator = Allocator::default();
    let parsed = parse(&allocator, &input, SourceType::script())?;
    if parsed.body.len() != 1
        || !matches!(parsed.body.first(), Some(Statement::ExpressionStatement(statement))
            if matches!(&statement.expression, Expression::ParenthesizedExpression(_)))
    {
        return Err(Error::Syntax("expected one provider expression".into()));
    }
    let mut names = Names::default();
    names.visit_program(&parsed);
    let (capture, error) = (names.fresh(), names.fresh());
    let code = format!(
        "try{{{capture}=[true,(\n{source}\n)];}}catch({error}){{{capture}=[false,{error}];}}"
    );
    Ok(Compiled { code, capture })
}

/// Parse the entire source without error recovery reaching the compiler.
fn parse<'a>(
    allocator: &'a Allocator,
    source: &'a str,
    kind: SourceType,
) -> Result<Program<'a>, Error> {
    let parsed = Parser::new(allocator, source, kind).parse();
    if let Some(error) = parsed.diagnostics.first() {
        return Err(Error::Syntax(error.to_string()));
    }
    Ok(parsed.program)
}

/// Read the block of the parsed asynchronous arrow used only for grammar admission.
fn function_body<'a>(program: &'a Program<'a>) -> Result<&'a FunctionBody<'a>, Error> {
    if program.body.len() == 1
        && let Some(Statement::ExpressionStatement(statement)) = program.body.first()
        && let Expression::ArrowFunctionExpression(arrow) = &statement.expression
        && let ArrowFunctionBody::FunctionBody(body) = &arrow.body
    {
        return Ok(body);
    }
    Err(Error::Syntax("expected one asynchronous body".into()))
}

/// Decoded identifiers from every scope, including free references and labels.
#[derive(Default)]
struct Names {
    /// All authored and already allocated spellings.
    used: HashSet<String>,
    /// Next candidate suffix.
    next: usize,
}

impl Names {
    /// Allocate a spelling that cannot capture an authored reference.
    fn fresh(&mut self) -> String {
        loop {
            let name = format!("__maestro{}", self.next);
            self.next += 1;
            if self.used.insert(name.clone()) {
                return name;
            }
        }
    }
}
impl<'a> Visit<'a> for Names {
    fn enter_node(&mut self, node: AstKind<'a>) {
        let name = match node {
            AstKind::IdentifierName(node) => node.name.as_str(),
            AstKind::IdentifierReference(node) => node.name.as_str(),
            AstKind::BindingIdentifier(node) => node.name.as_str(),
            AstKind::LabelIdentifier(node) => node.name.as_str(),
            AstKind::StringLiteral(node) => node.value.as_str().unwrap_or_default(),
            _ => return,
        };
        self.used.insert(name.to_owned());
    }
}

/// One source replacement, never overlapping another replacement.
struct Edit {
    /// Original parser byte range.
    span: Span,
    /// Browser-executable replacement.
    text: String,
}

/// Outer-body completion lowering with a saved pending value for each finalizer.
struct Completions<'a> {
    /// Private return-value binding.
    value: &'a str,
    /// Private exit label.
    exit: &'a str,
    /// Full parser input for expression slices.
    source: &'a str,
    /// Fresh names for finalizer-local saved values.
    names: &'a mut Names,
    /// Collected replacements.
    edits: Vec<Edit>,
    /// A source range that could not be represented.
    error: Option<Error>,
}
impl<'a> Visit<'a> for Completions<'_> {
    fn visit_return_statement(&mut self, statement: &ReturnStatement<'a>) {
        let expression = statement
            .argument
            .as_ref()
            .map_or(Ok("void 0"), |value| slice(self.source, value.span()));
        match expression {
            Ok(expression) => self.edits.push(Edit {
                span: statement.span,
                text: format!("{{{}=(0,({expression}));break {};}}", self.value, self.exit),
            }),
            Err(error) => self.error = Some(error),
        }
    }

    fn visit_try_statement(&mut self, statement: &TryStatement<'a>) {
        walk::walk_try_statement(self, statement);
        let Some(finalizer) = &statement.finalizer else {
            return;
        };
        let saved = self.names.fresh();
        let start = finalizer.span.start + 1;
        let end = finalizer.span.end - 1;
        self.edits.push(Edit {
            span: Span::new(start, start),
            text: format!("\nconst {saved}={};\n", self.value),
        });
        self.edits.push(Edit {
            span: Span::new(end, end),
            text: format!("\n;{}={saved};\n", self.value),
        });
    }

    fn visit_function_body(&mut self, _: &FunctionBody<'a>) {}

    fn visit_class(&mut self, _: &Class<'a>) {}
}

/// Instantiate function declarations before body evaluation while keeping function-scoped bindings.
fn hoist(
    statements: &[Statement<'_>],
    source: &str,
    edits: &mut Vec<Edit>,
) -> Result<String, Error> {
    let mut output = String::new();
    for statement in statements {
        let Statement::FunctionDeclaration(function) = statement else {
            continue;
        };
        let id = function.id.as_ref().ok_or(Error::SourceRange)?;
        let before = slice(source, Span::new(function.span.start, id.span.start))?;
        let after = slice(source, Span::new(id.span.end, function.span.end))?;
        // Writing to a String cannot fail.
        let _ = writeln!(output, "var {}=({before}{after});", id.name);
        edits.push(Edit {
            span: function.span,
            text: ";".into(),
        });
    }
    Ok(output)
}

/// Borrow a UTF-8 range produced by the parser.
fn slice(source: &str, span: Span) -> Result<&str, Error> {
    let start = usize::try_from(span.start).map_err(|_| Error::SourceRange)?;
    let end = usize::try_from(span.end).map_err(|_| Error::SourceRange)?;
    source.get(start..end).ok_or(Error::SourceRange)
}

/// Apply disjoint replacements and boundary insertions inside one supplied body.
fn replace(source: &str, range: Span, mut edits: Vec<Edit>) -> Result<String, Error> {
    edits.sort_by_key(|edit| (edit.span.start, edit.span.end));
    let mut output = String::new();
    let mut start = range.start;
    for edit in edits {
        if edit.span.start < start || edit.span.end > range.end {
            return Err(Error::SourceRange);
        }
        output.push_str(slice(source, Span::new(start, edit.span.start))?);
        output.push_str(&edit.text);
        start = edit.span.end;
    }
    output.push_str(slice(source, Span::new(start, range.end))?);
    Ok(output)
}
