use std::collections::{BTreeMap, BTreeSet};

use syn::{ext::IdentExt, parse::Parser, parse::discouraged::Speculative, spanned::Spanned};

use crate::source::Member;

/// Reject recognized optional polling macros, timer calls and scoped tracing captures in
/// test-owned parsed sources; only the test tracing owner may capture directly.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let mut diagnostics = Vec::new();
    for member in members {
        for source in &member.sources {
            diagnostics.extend(
                source
                    .walk
                    .iter()
                    .flat_map(|walk| &walk.time_limits)
                    .map(|(position, api)| (source.path.display().to_string(), *position, api)),
            );
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        diagnostics.sort_by(|left, right| (&left.0, left.1).cmp(&(&right.0, right.1)));
        Err(diagnostics
            .into_iter()
            .map(|(file, (line, _), api)| {
                match api {
                    Diagnostic::Timer(api) => format!("{file}:{line}: test-owned time limit: {api}; wait for completion instead"),
                    Diagnostic::OptionalPoll => format!("{file}:{line}: optional futures_util::poll! in test source; pin the future and call std::future::Future::poll once with std::task::Context::from_waker(std::task::Waker::noop())"),
                    Diagnostic::Capture(api) => format!("{file}:{line}: scoped tracing capture outside maestro-test-tracing: {api}; call maestro_test_tracing::with_default or with_subscriber instead"),
                }
            })
            .collect::<Vec<_>>()
            .join("\n"))
    }
}

#[cfg(test)]
thread_local! {
    /// Call paths and method names spelled by this check on the current test thread.
    static SPELLINGS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Recognized test-source portability or synchronization violation.
pub(crate) enum Diagnostic {
    /// Canonical timer identity.
    Timer(String),
    /// Single polling requires an optional dependency feature.
    OptionalPoll,
    /// Canonical scoped subscriber identity that bypasses the test tracing owner.
    Capture(String),
}

/// One occurrence in test code, in source order, resolved once the file's imports are known.
enum Pending<'a> {
    /// A finding that needs no import.
    Found((usize, usize), Diagnostic),
    /// A macro invocation path that may name the optional polling macro.
    Poll((usize, usize), String),
    /// A call through a path that may name a timer or a scoped capture.
    Call(&'a syn::ExprCall, String),
}

/// Imports and test-owned policy occurrences of one source, gathered by the shared syntax walk.
///
/// The walk passes every import, and test-owned syntax alone to the occurrence methods.
pub(crate) struct Calls<'a> {
    /// Use trees of the whole file, read only when an occurrence needs them.
    uses: Vec<&'a syn::UseTree>,
    /// Whether the source belongs to the test tracing owner, which may capture directly.
    tracing_owner: bool,
    /// Occurrences in source order.
    pending: Vec<Pending<'a>>,
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
            | "std"
            | "std::thread"
            | "futures_util"
            | "tracing"
            | "tracing::subscriber"
            | "tracing::dispatcher"
    ) || TIMERS.contains(&path.as_str())
        || OPTIONAL_POLL.contains(&path.as_str())
        || CAPTURES.contains(&path.as_str())
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

/// Prefer direct identities, then match every known alias candidate.
fn resolve<'a>(
    imports: &BTreeMap<String, BTreeSet<String>>,
    name: &str,
    known: &[&'a str],
) -> Option<&'a str> {
    if let Some(api) = known.iter().copied().find(|api| *api == name) {
        return Some(api);
    }
    let (first, suffix) = name.split_once("::").unwrap_or((name, ""));
    let candidates = imports.get(first)?;
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

impl<'a> Calls<'a> {
    /// Start the time-limit check of one member's source.
    pub(crate) fn new(member: &Member) -> Self {
        Self {
            uses: Vec::new(),
            tracing_owner: member.name == crate::graph::TEST_TRACING,
            pending: Vec::new(),
        }
    }

    /// Retain an import of the file; known aliases apply file-wide.
    pub(crate) fn item_use(&mut self, item: &'a syn::ItemUse) {
        self.uses.push(&item.tree);
    }

    /// Record a macro path at its authored invocation coordinates.
    fn optional_poll(&mut self, path: &syn::Path) {
        let position = path.span().start();
        self.pending.push(Pending::Poll(
            (position.line, position.column),
            path_name(path),
        ));
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

    /// Record a test-owned macro invocation and the literal invocations inside its tokens.
    pub(crate) fn macro_tokens(&mut self, invocation: &syn::Macro) {
        self.optional_poll(&invocation.path);
        self.nested_macros(invocation.tokens.clone());
    }

    /// Record a test-owned timed wait or scoped capture method.
    pub(crate) fn method_call(&mut self, call: &syn::ExprMethodCall) {
        #[cfg(test)]
        SPELLINGS.set(SPELLINGS.get() + 1);
        let name = call.method.unraw().to_string();
        if timed_wait(&name) {
            let position = call.span().start();
            self.pending.push(Pending::Found(
                (position.line, position.column),
                Diagnostic::Timer(name),
            ));
        } else if name == "with_subscriber" && !self.tracing_owner {
            let position = call.method.span().start();
            self.pending.push(Pending::Found(
                (position.line, position.column),
                Diagnostic::Capture("tracing::instrument::WithSubscriber::with_subscriber".into()),
            ));
        }
    }

    /// Record a test-owned call through a path, which may name a timer or a capture.
    pub(crate) fn call(&mut self, call: &'a syn::ExprCall) {
        let mut callee = &*call.func;
        while let syn::Expr::Paren(parenthesized) = callee {
            callee = &parenthesized.expr;
        }
        if let syn::Expr::Path(path) = callee {
            #[cfg(test)]
            SPELLINGS.set(SPELLINGS.get() + 1);
            self.pending
                .push(Pending::Call(call, path_name(&path.path)));
        }
    }

    /// Resolve the recorded occurrences against the file's imports, keeping source order.
    pub(crate) fn finish(self) -> Vec<((usize, usize), Diagnostic)> {
        let mut aliases = BTreeMap::new();
        // Every occurrence is test code, so a source without any reads no import.
        if !self.pending.is_empty() {
            for tree in self.uses {
                imports(tree, "", &mut aliases);
            }
        }
        let mut found = Vec::new();
        for pending in self.pending {
            match pending {
                Pending::Found(position, diagnostic) => found.push((position, diagnostic)),
                Pending::Poll(position, name)
                    if resolve(&aliases, &name, OPTIONAL_POLL).is_some() =>
                {
                    found.push((position, Diagnostic::OptionalPoll));
                }
                Pending::Poll(..) => {}
                Pending::Call(call, name) => {
                    let capturing = !self.tracing_owner;
                    found.extend(resolve_call(&aliases, call, &name, capturing));
                }
            }
        }
        found
    }
}

/// The timer and, where captures are reported, the scoped capture one called path names.
fn resolve_call(
    imports: &BTreeMap<String, BTreeSet<String>>,
    call: &syn::ExprCall,
    name: &str,
    capturing: bool,
) -> Vec<((usize, usize), Diagnostic)> {
    let position = || {
        let start = call.span().start();
        (start.line, start.column)
    };
    let mut found = Vec::new();
    let last = name.rsplit("::").next().unwrap_or(name);
    let timer = if timed_wait(last) {
        Some(last)
    } else {
        resolve(imports, name, TIMERS)
    };
    if let Some(api) = timer {
        found.push((position(), Diagnostic::Timer(api.into())));
    }
    if let Some(api) = resolve(imports, name, CAPTURES)
        && capturing
    {
        found.push((position(), Diagnostic::Capture(api.into())));
    }
    found
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

#[cfg(test)]
mod tests;
