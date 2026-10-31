//! Test reads of repository directories must be routed in the CI configuration.

use std::collections::BTreeSet;
use std::path::Path;

use proc_macro2::{TokenStream, TokenTree};
use syn::visit::Visit;

use crate::source::{Member, Source, inherit_test_context, test_scope};

/// Repository directories whose changes select only the packages that read them.
const DIRECTORIES: [&str; 3] = ["docs", "examples", "assets"];

/// Read the CI configuration's `test_inputs` routes, or `None` when the workspace has none.
fn routes(root: &Path) -> Result<Option<toml::Table>, String> {
    let path = root.join(".github/ci.toml");
    match std::fs::read_to_string(&path) {
        Ok(text) => text
            .parse::<toml::Table>()
            .map(Some)
            .map_err(|error| format!("{}: {error}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

/// Whether `test_inputs` routes `directory` (such as `docs/`) to `package`.
fn routed(configuration: &toml::Table, directory: &str, package: &str) -> bool {
    configuration
        .get("test_inputs")
        .and_then(|inputs| inputs.get(directory))
        .and_then(toml::Value::as_array)
        .is_some_and(|packages| packages.iter().any(|name| name.as_str() == Some(package)))
}

/// Reject a run-time test read of a repository directory that `test_inputs` does not route
/// to the reading package.
///
/// A test reads a directory when a string literal climbs out of the crate directory into it
/// (`../../assets/...`), or joins it onto a path in a file that climbs to the workspace root.
/// Compile-time `include_*!` arguments and attributes are not reads.
pub(crate) fn check(root: &Path, members: &[Member]) -> Result<(), String> {
    let Some(configuration) = routes(root)? else {
        return Ok(());
    };
    let diagnostics: BTreeSet<String> = members
        .iter()
        .flat_map(|member| {
            member
                .sources
                .iter()
                .map(move |source| (member, source))
        })
        .flat_map(|(member, source)| {
            reads(member, source)
                .into_iter()
                .filter(|(_, directory)| !routed(&configuration, directory, &member.name))
                .map(move |(line, directory)| {
                    format!(
                        "{}:{line}: tests read {directory} at run time, but test_inputs in .github/ci.toml does not route it to {}",
                        source.path.display(),
                        member.name
                    )
                })
        })
        .collect();
    if diagnostics.is_empty() {
        return Ok(());
    }
    Err(diagnostics.into_iter().collect::<Vec<_>>().join("\n"))
}

/// The located directory reads of one source, as `(line, "docs/")` pairs.
fn reads(member: &Member, source: &Source) -> BTreeSet<(usize, String)> {
    let Some(file) = source.syntax.as_ref() else {
        return BTreeSet::new();
    };
    let mut visitor = Reads {
        testing: test_scope(member, source),
        climbs: false,
        direct: BTreeSet::new(),
        joined: BTreeSet::new(),
    };
    visitor.visit_file(file);
    if visitor.climbs {
        visitor.direct.append(&mut visitor.joined);
    }
    visitor.direct
}

/// Directory reads found in one file.
struct Reads {
    /// Whether the current item inherits test ownership.
    testing: bool,
    /// Whether the file climbs out of its crate directory, by `..` segments or `ancestors`.
    climbs: bool,
    /// Reads whose literal itself climbs into the directory.
    direct: BTreeSet<(usize, String)>,
    /// Reads whose literal joins the directory onto some path.
    joined: BTreeSet<(usize, String)>,
}

impl Reads {
    /// Record a string literal found in test-owned code.
    fn literal(&mut self, value: &str, line: usize) {
        if !self.testing || !path_like(value) {
            return;
        }
        let segments: Vec<_> = value.split('/').collect();
        if let Some(index) = segments
            .windows(3)
            .position(|window| window[0] == ".." && window[1] == ".." && direct_name(window[2]))
        {
            self.direct
                .insert((line, format!("{}/", segments[index + 2])));
        }
        self.climbs |= segments.windows(2).any(|pair| pair == ["..", ".."]);
    }

    /// Record the literal that a path-joining call appends.
    fn joined(&mut self, value: &str, line: usize) {
        let first = value.split('/').next().unwrap_or_default();
        if self.testing && path_like(value) && direct_name(first) {
            self.joined.insert((line, format!("{first}/")));
        }
    }

    /// Record the string literals, joins and ancestor walks inside macro tokens.
    fn tokens(&mut self, tokens: &TokenStream) {
        let mut previous = None;
        for tree in tokens.clone() {
            self.token(&tree, previous.as_ref());
            previous = Some(tree);
        }
    }

    /// Record one macro token, given the token before it.
    fn token(&mut self, tree: &TokenTree, previous: Option<&TokenTree>) {
        match tree {
            TokenTree::Literal(literal) => {
                if let Some(value) = string_value(literal) {
                    self.literal(&value, literal.span().start().line);
                }
            }
            TokenTree::Ident(name) => self.climbs |= self.testing && name == "ancestors",
            TokenTree::Group(group) => {
                if previous.is_some_and(is_join) {
                    self.joined_argument(&group.stream());
                }
                self.tokens(&group.stream());
            }
            TokenTree::Punct(_) => {}
        }
    }

    /// Record the first argument of a join call written inside macro tokens.
    fn joined_argument(&mut self, arguments: &TokenStream) {
        if let Some(TokenTree::Literal(literal)) = arguments.clone().into_iter().next()
            && let Some(value) = string_value(&literal)
        {
            self.joined(&value, literal.span().start().line);
        }
    }
}

/// Whether a literal is a path or a path template rather than a line of source text.
fn path_like(value: &str) -> bool {
    !value.contains(['\n', '"'])
}

/// Whether `name` is one of the repository directories.
fn direct_name(name: &str) -> bool {
    DIRECTORIES.contains(&name)
}

/// Whether a token is the `join` or `push` name of a path-appending call.
fn is_join(tree: &TokenTree) -> bool {
    matches!(tree, TokenTree::Ident(name) if name == "join" || name == "push")
}

/// The decoded value of a string literal token.
fn string_value(literal: &proc_macro2::Literal) -> Option<String> {
    syn::parse_str::<syn::LitStr>(&literal.to_string())
        .ok()
        .map(|literal| literal.value())
}

impl<'ast> Visit<'ast> for Reads {
    fn visit_attribute(&mut self, _attribute: &'ast syn::Attribute) {}

    fn visit_expr_lit(&mut self, expression: &'ast syn::ExprLit) {
        if let syn::Lit::Str(literal) = &expression.lit {
            self.literal(&literal.value(), literal.span().start().line);
        }
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if self.testing && call.method == "ancestors" {
            self.climbs = true;
        }
        if (call.method == "join" || call.method == "push")
            && let Some(syn::Expr::Lit(expression)) = call.args.first()
            && let syn::Lit::Str(literal) = &expression.lit
        {
            self.joined(&literal.value(), literal.span().start().line);
        }
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        let compiled = invocation
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident.to_string().starts_with("include"));
        if !compiled {
            self.tokens(&invocation.tokens);
        }
    }

    inherit_test_context!(visit_item, Item, item_attributes);
    inherit_test_context!(visit_impl_item, ImplItem, impl_item_attributes);
    inherit_test_context!(visit_trait_item, TraitItem, trait_item_attributes);
}
