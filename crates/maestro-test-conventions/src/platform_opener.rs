//! Platform viewer executables belong to the application opener.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use proc_macro2::{TokenStream, TokenTree};
use syn::{parse::Parser, spanned::Spanned, visit::Visit};

use crate::source::{Member, covered, imports::flatten, test_scope};

/// Application source that owns native viewer selection.
const OWNER: &str = "src/utils/platform_opener.rs";

/// Reject viewer executable literals and shell-start selection outside their owner.
///
/// The whole-file token walk includes fragments, macro arguments, closures and methods.
/// Editor and clipboard commands have different operands and remain caller-owned.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let mut found = Vec::new();
    for member in members {
        for source in &member.sources {
            if test_scope(member, source)
                || (member.name == "maestro-app" && source.path == member.directory.join(OWNER))
            {
                continue;
            }
            let Ok(tokens) = source.contents.parse::<TokenStream>() else {
                continue;
            };
            let mut literals = Vec::new();
            strings(
                tokens,
                &BTreeSet::new(),
                source.test_spans(member),
                &BTreeMap::new(),
                &mut literals,
            );
            let shell_start = ["cmd", "/c", "start"]
                .iter()
                .all(|expected| literals.iter().any(|(text, _)| text == expected));
            found.extend(
                literals
                    .into_iter()
                    .filter(|(text, _)| {
                        matches!(text.as_str(), "open" | "xdg-open" | "explorer.exe")
                            || (shell_start && text == "cmd")
                    })
                    .map(|(_, line)| {
                        format!(
                            "{}:{line}: platform viewer outside utils::platform_opener",
                            source.path.display()
                        )
                    }),
            );
        }
    }
    if found.is_empty() {
        Ok(())
    } else {
        Err(found.join("\n"))
    }
}

#[cfg(test)]
thread_local! {
    /// Token positions looked up against test spans on the current test thread.
    static POSITION_LOOKUPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// Import maps built for token scopes on the current test thread.
    static IMPORT_SCOPES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Collect production literals, tracking local `open` bindings through nested groups.
fn strings(
    tokens: TokenStream,
    inherited: &BTreeSet<String>,
    testing: &[Range<(usize, usize)>],
    inherited_imports: &BTreeMap<String, BTreeSet<String>>,
    found: &mut Vec<(String, usize)>,
) {
    let tokens: Vec<_> = tokens
        .into_iter()
        .filter(|token| testing.is_empty() || !covered(testing, position(token)))
        .collect();
    let imports = scoped_imports(&tokens, inherited_imports);
    let mut bindings = inherited.clone();
    let mut command = false;
    let mut local = None;
    let mut skip = 0;
    for (index, token) in tokens.iter().enumerate() {
        if index < skip {
            continue;
        }
        if let Some((count, body, captured)) = closure_body(&tokens[index..], &bindings) {
            strings(body, &captured, testing, &imports, found);
            skip = index + count;
            command = false;
            continue;
        }
        match token {
            TokenTree::Ident(name) => {
                if name == "let" {
                    local = Some(index + 1);
                }
                command = name == "Command" || name == "command" || (command && name == "new");
            }
            TokenTree::Punct(punct) => {
                if punct.as_char() == ';'
                    && let Some(start) = local.take()
                {
                    local_binding(&tokens[start..index], &mut bindings);
                }
                command &= punct.as_char() == ':';
            }
            TokenTree::Literal(_) => {
                if let Some(text) = literal_text(token)
                    && text != "open"
                {
                    found.push((text, token.span().start().line));
                }
                command = false;
            }
            TokenTree::Group(group) if cli_name(&tokens[..index], group.stream(), &imports) => {
                command = false;
            }
            TokenTree::Group(group) => {
                if command && let Some(line) = open_operand(group.stream(), &bindings) {
                    found.push(("open".into(), line));
                }
                strings(group.stream(), &bindings, testing, &imports, found);
                command = false;
            }
        }
    }
}

/// Locate a token's start in the line and column coordinates of syntax spans.
fn position(token: &TokenTree) -> (usize, usize) {
    #[cfg(test)]
    POSITION_LOOKUPS.set(POSITION_LOOKUPS.get() + 1);
    let start = token.span().start();
    (start.line, start.column)
}

/// Resolve explicit imports in this token group without changing the enclosing scope.
fn scoped_imports<'a>(
    tokens: &[TokenTree],
    inherited: &'a BTreeMap<String, BTreeSet<String>>,
) -> Cow<'a, BTreeMap<String, BTreeSet<String>>> {
    let mut imports = BTreeMap::<String, BTreeSet<String>>::new();
    for (index, token) in tokens.iter().enumerate() {
        if !matches!(token, TokenTree::Ident(name) if name == "use") {
            continue;
        }
        let Some(end) = tokens[index..]
            .iter()
            .position(|token| matches!(token, TokenTree::Punct(punct) if punct.as_char() == ';'))
        else {
            continue;
        };
        let item = tokens[index..=index + end].iter().cloned().collect();
        let Ok(item) = syn::parse2::<syn::ItemUse>(item) else {
            continue;
        };
        for import in flatten(&item.tree) {
            if let Some(binding) = import.binding {
                let path = import
                    .path
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>();
                let path = path.join("::");
                imports
                    .entry(binding.to_string())
                    .or_default()
                    .insert(path.trim_end_matches("::self").into());
            }
        }
    }
    if imports.is_empty() {
        return Cow::Borrowed(inherited);
    }
    #[cfg(test)]
    IMPORT_SCOPES.set(IMPORT_SCOPES.get() + 1);
    let mut scoped = inherited.clone();
    scoped.extend(imports);
    Cow::Owned(scoped)
}

/// Recognize one plain name supplied to a known command-line parser constructor.
fn cli_name(
    before: &[TokenTree],
    operands: TokenStream,
    imports: &BTreeMap<String, BTreeSet<String>>,
) -> bool {
    let mut operands = operands.into_iter();
    if !matches!(
        operands.next(),
        Some(TokenTree::Literal(_) | TokenTree::Ident(_))
    ) || !operands.next().is_none_or(|token| {
        matches!(token, TokenTree::Punct(punct) if punct.as_char() == ',')
            && operands.next().is_none()
    }) {
        return false;
    }
    let start = before
        .iter()
        .rposition(|token| {
            !matches!(token, TokenTree::Ident(_))
                && !matches!(token, TokenTree::Punct(punct) if punct.as_char() == ':')
        })
        .map_or(0, |index| index + 1);
    let Ok(path) = syn::parse2::<syn::Path>(before[start..].iter().cloned().collect()) else {
        return false;
    };
    let parts: Vec<_> = path
        .segments
        .iter()
        .map(|part| part.ident.to_string())
        .collect();
    let name = parts.join("::");
    let (first, rest) = name.split_once("::").unwrap_or((&name, ""));
    imports.get(first).map_or_else(
        || name == "clap::Command::new",
        |prefixes| {
            prefixes
                .iter()
                .all(|prefix| format!("{prefix}::{rest}") == "clap::Command::new")
        },
    )
}

/// Scan a closure body with only its unshadowed captures, leaving outer bindings intact.
fn closure_body(
    tokens: &[TokenTree],
    inherited: &BTreeSet<String>,
) -> Option<(usize, TokenStream, BTreeSet<String>)> {
    if !matches!(tokens.first(), Some(TokenTree::Punct(punct)) if punct.as_char() == '|') {
        return None;
    }
    let parser = |input: syn::parse::ParseStream<'_>| {
        let closure = input.parse::<syn::ExprClosure>()?;
        let rest = input.parse::<TokenStream>()?;
        Ok((closure, rest.into_iter().count()))
    };
    let (closure, remaining) = parser.parse2(tokens.iter().cloned().collect()).ok()?;
    let count = tokens.len() - remaining;
    let start = closure.body.span().start();
    let body = tokens[..count]
        .iter()
        .filter(|token| token.span().start() >= start)
        .cloned()
        .collect();
    let mut captured = inherited.clone();
    for parameter in &closure.inputs {
        Parameters(&mut captured).visit_pat(parameter);
    }
    Some((count, body, captured))
}

/// Remove parameter-bound identifiers, including names inside destructuring patterns.
struct Parameters<'a>(&'a mut BTreeSet<String>);

impl<'ast> Visit<'ast> for Parameters<'_> {
    fn visit_pat_ident(&mut self, pattern: &'ast syn::PatIdent) {
        self.0.remove(&pattern.ident.to_string());
        syn::visit::visit_pat_ident(self, pattern);
    }
}

/// Decode a string token without treating other literal kinds as executable names.
fn literal_text(token: &TokenTree) -> Option<String> {
    let TokenTree::Literal(literal) = token else {
        return None;
    };
    syn::parse_str::<syn::LitStr>(&literal.to_string())
        .ok()
        .map(|text| text.value())
}

/// Apply a simple local binding after its initializer has been scanned.
fn local_binding(tokens: &[TokenTree], bindings: &mut BTreeSet<String>) {
    let mut tokens = tokens.iter();
    let name = tokens.next();
    let name = match name {
        Some(TokenTree::Ident(name)) if name == "mut" => tokens.next(),
        name => name,
    };
    let Some(TokenTree::Ident(name)) = name else {
        return;
    };
    let name = name.to_string();
    bindings.remove(&name);
    let initializer = tokens
        .take_while(|token| !matches!(token, TokenTree::Punct(punct) if punct.as_char() == ';'))
        .skip_while(|token| !matches!(token, TokenTree::Punct(punct) if punct.as_char() == '='))
        .nth(1);
    if initializer.and_then(literal_text).as_deref() == Some("open") {
        bindings.insert(name);
    }
}

/// Recognize a literal or locally bound macOS executable passed to command construction.
fn open_operand(tokens: TokenStream, bindings: &BTreeSet<String>) -> Option<usize> {
    let operand = tokens.into_iter().next()?;
    let bound = matches!(&operand, TokenTree::Ident(name) if bindings.contains(&name.to_string()));
    (bound || literal_text(&operand).as_deref() == Some("open"))
        .then(|| operand.span().start().line)
}

#[cfg(test)]
mod tests;
