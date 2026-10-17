//! Bounded syntax checks for exact integration-test child runs.

use std::collections::BTreeMap;

use syn::{Expr, visit::Visit};

/// Check each function independently, including non-test child-runner helpers.
pub(super) fn check(file: &syn::File) -> Result<(), String> {
    let mut functions = Functions::default();
    functions.visit_file(file);
    functions.error.map_or(Ok(()), Err)
}

/// Visit functions without attributing a neighbour's witness to a child runner.
#[derive(Default)]
struct Functions {
    /// First child-runner diagnostic.
    error: Option<String>,
}

impl<'ast> Visit<'ast> for Functions {
    fn visit_item_fn(&mut self, function: &'ast syn::ItemFn) {
        let mut child = Child::default();
        child.visit_block(&function.block);
        if child.exact > 0
            && (child.targets.len() != child.exact
                || !child.targets.iter().all(|target| {
                    target
                        .as_ref()
                        .is_some_and(|target| child.witnesses.contains(target))
                }))
        {
            self.error = Some(format!(
                "{}: --exact children require a qualified literal or child_process::selector and child_process::assert_executed or an asserted stdout success line",
                function.sig.ident
            ));
        }
        syn::visit::visit_item_fn(self, function);
    }
}

/// A literal libtest identity or the local binding produced by the shared selector.
#[derive(Clone, PartialEq)]
enum Selection {
    /// A module-qualified literal test name.
    Qualified(String),
    /// A shared-selector result, identified by its local binding.
    Current(String),
}

/// Selection identities and execution witnesses within one function.
#[derive(Default)]
struct Child {
    /// Local selection bindings.
    names: BTreeMap<String, Selection>,
    /// Number of exact-selection literals encountered outside macros.
    exact: usize,
    /// Qualified selections passed immediately after an exact flag.
    targets: Vec<Option<Selection>>,
    /// Selections whose execution is asserted.
    witnesses: Vec<Selection>,
}

impl Child {
    /// Resolve literals, local bindings and the shared selector, never arbitrary expressions.
    fn name(&self, expression: &Expr) -> Option<Selection> {
        match expression {
            Expr::Lit(value) => match &value.lit {
                syn::Lit::Str(value) if qualified(&value.value()) => {
                    Some(Selection::Qualified(value.value()))
                }
                _ => None,
            },
            Expr::Path(value) => value
                .path
                .get_ident()
                .and_then(|name| self.names.get(&name.to_string()).cloned()),
            Expr::Reference(value) => self.name(&value.expr),
            Expr::Try(value) => self.name(&value.expr),
            Expr::MethodCall(value) if value.method == "unwrap" => self.name(&value.receiver),
            Expr::Call(value) if shared(&value.func, "selector") => {
                Some(Selection::Current(String::new()))
            }
            _ => None,
        }
    }

    /// Record only explicit stdout contains assertions with the complete libtest success line.
    fn stdout_witness(&mut self, expression: &Expr) {
        let Expr::MethodCall(call) = expression else {
            return;
        };
        if call.method != "contains" || !stdout(&call.receiver) {
            return;
        }
        let Some(argument) = call.args.first() else {
            return;
        };
        let argument = match argument {
            Expr::Reference(reference) => reference.expr.as_ref(),
            argument => argument,
        };
        if let Expr::Lit(value) = argument
            && let syn::Lit::Str(value) = &value.lit
        {
            let line = value.value();
            if let Some(name) = line
                .strip_prefix("test ")
                .and_then(|line| line.strip_suffix(" ... ok"))
                .filter(|name| qualified(name))
            {
                self.witnesses.push(Selection::Qualified(name.into()));
            }
        }
        if let Expr::Macro(value) = argument
            && value.mac.path.is_ident("format")
            && let Ok(format) = syn::parse2::<syn::LitStr>(value.mac.tokens.clone())
        {
            self.witnesses.extend(
                self.names
                    .iter()
                    .filter(|(binding, _)| format.value() == format!("test {{{binding}}} ... ok"))
                    .map(|(_, name)| name.clone()),
            );
        }
    }
}

impl<'ast> Visit<'ast> for Child {
    fn visit_item_fn(&mut self, _function: &'ast syn::ItemFn) {}

    fn visit_local(&mut self, local: &'ast syn::Local) {
        if let (syn::Pat::Ident(binding), Some(initializer)) = (&local.pat, &local.init)
            && let Some(mut name) = self.name(&initializer.expr)
        {
            if matches!(name, Selection::Current(_)) {
                name = Selection::Current(binding.ident.to_string());
            }
            self.names.insert(binding.ident.to_string(), name);
        }
        syn::visit::visit_local(self, local);
    }

    fn visit_lit_str(&mut self, literal: &'ast syn::LitStr) {
        self.exact += usize::from(literal.value() == "--exact");
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if call.method == "args"
            && let Some(Expr::Array(arguments)) = call.args.first()
        {
            let arguments: Vec<_> = arguments.elems.iter().collect();
            self.targets.extend(arguments.windows(2)
                .filter(|pair| matches!(pair[0], Expr::Lit(value) if matches!(&value.lit, syn::Lit::Str(value) if value.value() == "--exact")))
                .map(|pair| self.name(pair[1])).collect::<Vec<_>>());
        }
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if shared(&call.func, "assert_executed")
            && let Some(name) = call.args.iter().nth(1).and_then(|name| self.name(name))
        {
            self.witnesses.push(name);
        }
        syn::visit::visit_expr_call(self, call);
    }

    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        if invocation.path.is_ident("assert") {
            // An assertion's first expression is the observed condition; messages are not witnesses.
            use syn::parse::Parser as _;
            let parser = |input: syn::parse::ParseStream<'_>| {
                let expression = input.parse::<Expr>()?;
                let _: proc_macro2::TokenStream = input.parse()?;
                Ok(expression)
            };
            if let Ok(expression) = parser.parse2(invocation.tokens.clone()) {
                self.stdout_witness(&expression);
            }
        }
    }
}

/// Recognize the two reused helper entry points, not unrelated same-named calls.
fn shared(expression: &Expr, name: &str) -> bool {
    let Expr::Path(path) = expression else {
        return false;
    };
    let mut segments = path.path.segments.iter().rev();
    segments.next().is_some_and(|segment| segment.ident == name)
        && segments
            .next()
            .is_some_and(|segment| segment.ident == "child_process")
}

/// Recognize a module-qualified libtest name.
fn qualified(name: &str) -> bool {
    name.contains("::")
        && name
            .split("::")
            .all(|part| syn::parse_str::<syn::Ident>(part).is_ok())
}

/// Recognize `String::from_utf8_lossy(&output.stdout)`, not stderr or an arbitrary string.
fn stdout(expression: &Expr) -> bool {
    let Expr::Call(call) = expression else {
        return false;
    };
    let Expr::Path(path) = call.func.as_ref() else {
        return false;
    };
    let segments: Vec<_> = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect();
    if !matches!(segments.as_slice(), [owner, method] if owner == "String" && method == "from_utf8_lossy")
        && !matches!(segments.as_slice(), [root, module, owner, method] if root == "std" && module == "string" && owner == "String" && method == "from_utf8_lossy")
    {
        return false;
    }
    matches!(call.args.first(), Some(Expr::Reference(reference)) if matches!(reference.expr.as_ref(), Expr::Field(field) if matches!(&field.member, syn::Member::Named(name) if name == "stdout")))
}
