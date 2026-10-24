use std::collections::{BTreeMap, BTreeSet};

use syn::{
    ext::IdentExt, parse::Parser, parse::discouraged::Speculative, spanned::Spanned, visit::Visit,
};

use crate::source::{Member, inherit_test_context, test_scope};

/// Reject recognized optional polling macros, timer calls and scoped tracing captures in
/// test-owned parsed sources; only the test tracing owner may capture directly.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let mut diagnostics = Vec::new();
    for member in members {
        for source in &member.sources {
            let Some(syntax) = &source.syntax else {
                continue;
            };
            let mut aliases = Aliases::default();
            aliases.visit_file(syntax);
            let mut calls = Calls {
                testing: test_scope(member, source),
                tracing_owner: member.name == crate::graph::TEST_TRACING,
                ftp: Ftp::of(&source.path),
                found: Vec::new(),
                imports: aliases.0,
            };
            calls.visit_file(syntax);
            diagnostics.extend(calls.found.into_iter().map(|(position, api)| {
                let message = match api {
                    Diagnostic::Timer(api) => format!(
                        "test-owned time limit: {api}; wait for completion instead"
                    ),
                    Diagnostic::OptionalPoll => String::from(
                        "optional futures_util::poll! in test source; pin the future and call std::future::Future::poll once with std::task::Context::from_waker(std::task::Waker::noop())",
                    ),
                    Diagnostic::Capture(api) => format!(
                        "scoped tracing capture outside maestro-test-tracing: {api}; call maestro_test_tracing::with_default or with_subscriber instead"
                    ),
                    Diagnostic::Ftp(message) => message.to_owned(),
                };
                (source.path.display().to_string(), position.0, message)
            }));
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        diagnostics.sort_by(|left, right| (&left.0, left.1).cmp(&(&right.0, right.1)));
        Err(diagnostics
            .into_iter()
            .map(|(file, line, message)| format!("{file}:{line}: {message}"))
            .collect::<Vec<_>>()
            .join("\n"))
    }
}

/// Recognized test-source portability or synchronization violation.
enum Diagnostic {
    /// Canonical timer identity.
    Timer(String),
    /// Single polling requires an optional dependency feature.
    OptionalPoll,
    /// Canonical scoped subscriber identity that bypasses the test tracing owner.
    Capture(String),
    /// FTP ownership violation, which the message states.
    Ftp(&'static str),
}

/// The FTP activity owner, the file allowed to wait on or discard a socket operation.
const OWNER: &str = "ftp_context.rs";

/// What the FTP ownership policy expects of one source.
enum Ftp {
    /// The source is outside the FTP client tree.
    Outside,
    /// The source is in the FTP client tree, but is not the activity owner.
    Guarded,
    /// The source is the activity owner.
    Owner,
}

impl Ftp {
    /// Classify a source by its path.
    fn of(path: &std::path::Path) -> Self {
        let text = path.to_string_lossy();
        if !text.contains("get_uri/ftp") {
            return Self::Outside;
        }
        match path.file_name().map(|name| name.to_string_lossy()) {
            Some(name) if name == OWNER => Self::Owner,
            _ => Self::Guarded,
        }
    }
}

/// Test ownership and located policy occurrences for one source.
struct Calls {
    /// Imported local names and their known authored identities.
    imports: BTreeMap<String, BTreeSet<String>>,
    /// Whether the current syntax belongs to tests.
    testing: bool,
    /// Whether the source belongs to the test tracing owner, which may capture directly.
    tracing_owner: bool,
    /// What the FTP ownership policy expects of this source.
    ftp: Ftp,
    /// Canonical labels with source coordinates.
    found: Vec<((usize, usize), Diagnostic)>,
}

/// Collect known import candidates from a use tree.
fn imports(tree: &syn::UseTree, prefix: &str, names: &mut BTreeMap<String, BTreeSet<String>>) {
    match tree {
        syn::UseTree::Path(path) => imports(
            &path.tree,
            &format!("{prefix}{}::", path.ident.unraw()),
            names,
        ),
        syn::UseTree::Name(name) => bind(names, &name.ident, import_path(prefix, &name.ident)),
        syn::UseTree::Rename(name) => bind(names, &name.rename, import_path(prefix, &name.ident)),
        syn::UseTree::Group(group) => {
            for tree in &group.items {
                imports(tree, prefix, names);
            }
        }
        syn::UseTree::Glob(_) => {
            for api in TIMERS.iter().chain(OPTIONAL_POLL).chain(CAPTURES) {
                if api
                    .rsplit_once("::")
                    .is_some_and(|(module, _)| module == prefix.trim_end_matches("::"))
                {
                    names
                        .entry(api.rsplit("::").next().unwrap_or(api).into())
                        .or_default()
                        .insert((*api).into());
                }
            }
        }
    }
}

/// Retain aliases of known timer functions, optional macros, captures or their namespaces.
fn bind(names: &mut BTreeMap<String, BTreeSet<String>>, name: &syn::Ident, path: String) {
    if matches!(
        path.as_str(),
        "tokio"
            | "tokio::time"
            | "tokio::io"
            | "std"
            | "std::thread"
            | "futures_util"
            | "tracing"
            | "tracing::subscriber"
            | "tracing::dispatcher"
    ) || TIMERS.contains(&path.as_str())
        || OPTIONAL_POLL.contains(&path.as_str())
        || CAPTURES.contains(&path.as_str())
        || FTP_OPERATIONS.iter().any(|operation| {
            operation
                .strip_prefix(path.as_str())
                .is_some_and(|rest| rest.starts_with("::"))
        })
    {
        names
            .entry(name.unraw().to_string())
            .or_default()
            .insert(path);
    }
}

/// Canonical free-function timer identities.
const TIMERS: &[&str] = &[
    "tokio::time::timeout",
    "tokio::time::timeout_at",
    "std::thread::park_timeout",
];

/// Canonical socket operations, which only the FTP activity owner may perform.
const FTP_OPERATIONS: &[&str] = &[
    "tokio::io::AsyncReadExt::read",
    "tokio::io::AsyncWriteExt::write_all",
];

/// Canonical optional single-poll macro identity.
const OPTIONAL_POLL: &[&str] = &["futures_util::poll"];

/// Canonical scoped subscriber functions the test tracing owner wraps.
const CAPTURES: &[&str] = &[
    "tracing::subscriber::with_default",
    "tracing::subscriber::set_default",
    "tracing::dispatcher::with_default",
    "tracing::dispatcher::set_default",
];

/// Resolve a self import to its containing namespace.
fn import_path(prefix: &str, name: &syn::Ident) -> String {
    if name == "self" {
        prefix.trim_end_matches("::").into()
    } else {
        format!("{prefix}{}", name.unraw())
    }
}

/// Known API and namespace aliases collected across a parsed file.
#[derive(Default)]
struct Aliases(BTreeMap<String, BTreeSet<String>>);

impl<'ast> Visit<'ast> for Aliases {
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        imports(&item.tree, "", &mut self.0);
    }
}

impl Calls {
    /// Prefer direct identities, then match every known alias candidate.
    fn resolve<'a>(&self, name: &str, known: &[&'a str]) -> Option<&'a str> {
        if let Some(api) = known.iter().copied().find(|api| *api == name) {
            return Some(api);
        }
        let (first, suffix) = name.split_once("::").unwrap_or((name, ""));
        let candidates = self.imports.get(first)?;
        known.iter().copied().find(|api| {
            candidates.iter().any(|candidate| {
                (suffix.is_empty() && *api == candidate)
                    || api
                        .strip_prefix(candidate.as_str())
                        .and_then(|rest| rest.strip_prefix("::"))
                        == Some(suffix)
            })
        })
    }

    /// Record an optional macro at its authored invocation coordinates.
    fn optional_poll(&mut self, path: &syn::Path) {
        if self.resolve(&path_name(path), OPTIONAL_POLL).is_some() {
            let position = path.span().start();
            self.found
                .push(((position.line, position.column), Diagnostic::OptionalPoll));
        }
    }

    /// Record an FTP ownership violation.
    fn ftp(&mut self, message: &'static str, position: proc_macro2::LineColumn) {
        if matches!(self.ftp, Ftp::Guarded) {
            self.found
                .push(((position.line, position.column), Diagnostic::Ftp(message)));
        }
    }

    /// Record a scoped capture in test-owned syntax outside the test tracing owner.
    fn capture(&mut self, position: proc_macro2::LineColumn, api: &str) {
        if self.testing && !self.tracing_owner {
            self.found.push((
                (position.line, position.column),
                Diagnostic::Capture(api.into()),
            ));
        }
    }

    /// Inspect literal invocation tokens without expanding their outer macro.
    fn nested_macros(&mut self, tokens: proc_macro2::TokenStream) {
        let parser = |input: syn::parse::ParseStream<'_>| self.scan_macro_tokens(input);
        let _ = parser.parse2(tokens);
    }

    /// Consume every token tree, descending into groups and literal invocations.
    fn scan_macro_tokens(&mut self, input: syn::parse::ParseStream<'_>) -> syn::Result<()> {
        while !input.is_empty() {
            let candidate = input.fork();
            let Ok(path) = candidate.call(syn::Path::parse_mod_style) else {
                self.nested_group(input.parse()?);
                continue;
            };
            input.advance_to(&candidate);
            if candidate.parse::<syn::Token![!]>().is_ok()
                && let Ok(group) = candidate.parse::<proc_macro2::Group>()
            {
                self.optional_poll(&path);
                self.nested_macros(group.stream());
                input.advance_to(&candidate);
            }
        }
        Ok(())
    }

    /// Descend into grouped tokens while leaving literals opaque.
    fn nested_group(&mut self, token: proc_macro2::TokenTree) {
        if let proc_macro2::TokenTree::Group(group) = token {
            self.nested_macros(group.stream());
        }
    }
}

impl<'ast> Visit<'ast> for Calls {
    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        if self.testing {
            self.optional_poll(&invocation.path);
            self.nested_macros(invocation.tokens.clone());
        }
        if let Some(last) = invocation.path.segments.last()
            && matches!(
                last.ident.unraw().to_string().as_str(),
                "println"
                    | "eprintln"
                    | "print"
                    | "dbg"
                    | "debug"
                    | "info"
                    | "warn"
                    | "error"
                    | "trace"
            )
        {
            self.ftp(LOGGER, invocation.span().start());
        }
        syn::visit::visit_macro(self, invocation);
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        if let syn::Pat::Wild(_) = &local.pat
            && let Some(init) = &local.init
            && discards_assignment(&init.expr)
        {
            self.ftp(DISCARDED, local.span().start());
        }
        syn::visit::visit_local(self, local);
    }

    fn visit_stmt(&mut self, statement: &'ast syn::Stmt) {
        if let syn::Stmt::Expr(expression, Some(_)) = statement
            && discards_answer(expression)
        {
            self.ftp(DISCARDED, statement.span().start());
        }
        syn::visit::visit_stmt(self, statement);
    }

    inherit_test_context!(visit_item, Item, item_attributes);
    inherit_test_context!(visit_impl_item, ImplItem, impl_item_attributes);
    inherit_test_context!(visit_trait_item, TraitItem, trait_item_attributes);
    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        let name = call.method.unraw().to_string();
        if self.testing && timed_wait(&name) {
            let position = call.span().start();
            self.found
                .push(((position.line, position.column), Diagnostic::Timer(name)));
        } else if name == "with_subscriber" {
            self.capture(
                call.method.span().start(),
                "tracing::instrument::WithSubscriber::with_subscriber",
            );
        }
        syn::visit::visit_expr_method_call(self, call);
    }
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        let mut callee = &*call.func;
        while let syn::Expr::Paren(parenthesized) = callee {
            callee = &parenthesized.expr;
        }
        if let syn::Expr::Path(path) = callee {
            let name = path_name(&path.path);
            let last = name.rsplit("::").next().unwrap_or(&name);
            let timer = if timed_wait(last) {
                Some(last)
            } else {
                self.resolve(&name, TIMERS)
            };
            if self.testing
                && let Some(api) = timer
            {
                let position = call.span().start();
                self.found.push((
                    (position.line, position.column),
                    Diagnostic::Timer(api.into()),
                ));
            }
            let waits = timer.is_some()
                || matches!(last, "sleep" | "sleep_until" | "connect")
                || name.ends_with("AsyncReadExt::read")
                || name.ends_with("AsyncWriteExt::write_all")
                || self.resolve(&name, FTP_OPERATIONS).is_some();
            if waits {
                self.ftp(WAIT_OUTSIDE, call.span().start());
            }
            if let Some(api) = self.resolve(&name, CAPTURES) {
                self.capture(call.span().start(), api);
            }
        }
        syn::visit::visit_expr_call(self, call);
    }
}

/// The diagnostic for a socket wait in the FTP tree outside its activity owner.
const WAIT_OUTSIDE: &str = "FTP socket wait outside the activity owner";

/// The diagnostic for a discarded FTP activity result.
const DISCARDED: &str = "FTP activity result discarded";

/// The diagnostic for a command or credential logger in the FTP tree.
const LOGGER: &str = "FTP command or credential logger";

/// Whether the expression throws away the result of a call in an assignment.
fn discards_assignment(expression: &syn::Expr) -> bool {
    match expression {
        syn::Expr::Await(awaited) => matches!(&*awaited.base, syn::Expr::MethodCall(_)),
        syn::Expr::MethodCall(_) => true,
        _ => false,
    }
}

/// Whether the statement throws away an awaited request's answer, as `request(..).await.ok();`
/// does.
fn discards_answer(expression: &syn::Expr) -> bool {
    let syn::Expr::MethodCall(discard) = expression else {
        return false;
    };
    if discard.method.unraw() != "ok" {
        return false;
    }
    let syn::Expr::Await(awaited) = &*discard.receiver else {
        return false;
    };
    let syn::Expr::MethodCall(request) = &*awaited.base else {
        return false;
    };
    matches!(
        request.method.unraw().to_string().as_str(),
        "request" | "request_ignoring_error"
    )
}

/// Recognize the syntactically forbidden timed method names.
fn timed_wait(name: &str) -> bool {
    matches!(
        name,
        "recv_timeout" | "recv_deadline" | "wait_timeout" | "wait_timeout_while"
    )
}

/// Normalize raw identifiers without changing authored source coordinates.
fn path_name(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|part| part.ident.unraw().to_string())
        .collect::<Vec<_>>()
        .join("::")
}
