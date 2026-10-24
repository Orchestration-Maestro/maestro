//! One owner for the session directory name.

use crate::source::{Member, Source, test_layout};
use std::path::Path;
use syn::visit::Visit;
use syn::{Expr, ExprCall, ExprMacro, Pat, Stmt};

/// The crate that owns the encoder.
const OWNER_CRATE: &str = "maestro-session";
/// The encoder's own module.
const OWNER_FILE: &str = "src/session_manager/files.rs";
/// The published encoder name.
const ENCODER: &str = "encode_session_dir";
/// Every production file that must reach the encoder instead of spelling the name.
const CONSUMERS: [(&str, &str); 4] = [
    ("maestro-app", "src/developer_tools/cost.rs"),
    ("maestro-app", "src/developer_tools/stats.rs"),
    (
        "maestro-app",
        "src/developer_tools/session_transcripts/mod.rs",
    ),
    ("maestro-session", "src/session_manager/files.rs"),
];

/// Reject a second session directory encoder and require every selector to delegate.
///
/// The check reads parsed syntax: a comment or a string that spells the call
/// does not count, and a call whose result the statement throws away is
/// rejected, because only a selector that keeps the owner's result delegates.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let mut owned = false;
    for member in members {
        for source in &member.sources {
            let relative = relative_path(&source.path, &member.directory);
            if !relative.starts_with("src/") || test_layout(&source.path, &member.directory) {
                continue;
            }
            if member.name == OWNER_CRATE && relative == OWNER_FILE {
                definitions(&source.contents)?;
                owned = true;
            } else if let Some(line) = copy(&source.contents) {
                return Err(format!(
                    "{}:{line}: session directory encoding belongs to {OWNER_CRATE}::{ENCODER}",
                    source.path.display()
                ));
            }
        }
    }
    if !owned {
        return Ok(());
    }
    for (member, relative) in CONSUMERS {
        let Some(found) = consumer(members, member, relative)? else {
            continue;
        };
        let delegation = Delegation::of(found)?;
        if !delegation.calls {
            return Err(format!(
                "{}: session directory selector does not call {ENCODER}",
                found.path.display()
            ));
        }
        if delegation.discarded {
            return Err(format!(
                "{}: session directory selector discards the {ENCODER} result",
                found.path.display()
            ));
        }
        // The owner's own module spells the name; every other consumer must not.
        if delegation.copied && !(member == OWNER_CRATE && relative == OWNER_FILE) {
            return Err(format!(
                "{}: session directory selector spells the directory name locally",
                found.path.display()
            ));
        }
    }
    Ok(())
}

/// The member source spelling one consumer, when that crate is present.
fn consumer<'a>(
    members: &'a [Member],
    member: &str,
    relative: &str,
) -> Result<Option<&'a Source>, String> {
    let Some(found) = members.iter().find(|found| found.name == member) else {
        return Ok(None);
    };
    found
        .sources
        .iter()
        .find(|source| relative_path(&source.path, &found.directory) == relative)
        .map(Some)
        .ok_or_else(|| format!("{member}: missing session directory selector {relative}"))
}

/// The owner calls one selector makes, and whether one throws its result away.
#[derive(Default)]
struct Delegation {
    /// A statement or expression calls the owner.
    calls: bool,
    /// A statement drops the owner's result instead of using it.
    discarded: bool,
    /// A macro spells the directory name itself, across any number of lines.
    copied: bool,
}

impl Delegation {
    /// Read one parsed selector source.
    fn of(source: &Source) -> Result<Self, String> {
        let Some(syntax) = &source.syntax else {
            return Err(format!(
                "{}: session directory selector does not parse",
                source.path.display()
            ));
        };
        let mut delegation = Self::default();
        delegation.visit_file(syntax);
        Ok(delegation)
    }
}

impl<'ast> Visit<'ast> for Delegation {
    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        if names_encoder(&node.func) {
            self.calls = true;
        }
        syn::visit::visit_expr_call(self, node);
    }

    fn visit_expr_macro(&mut self, node: &'ast ExprMacro) {
        if node
            .mac
            .tokens
            .clone()
            .into_iter()
            .any(|token| literal_spells_encoding(&token))
        {
            self.copied = true;
        }
        syn::visit::visit_expr_macro(self, node);
    }

    fn visit_stmt(&mut self, node: &'ast Stmt) {
        let thrown_away = match node {
            Stmt::Expr(expression, Some(_)) => is_call(expression),
            Stmt::Local(local) => {
                matches!(local.pat, Pat::Wild(_))
                    && local.init.as_ref().is_some_and(|init| is_call(&init.expr))
            }
            _ => false,
        };
        if thrown_away {
            self.discarded = true;
        }
        syn::visit::visit_stmt(self, node);
    }
}

/// Whether a token is a string literal opening and closing with the wrapper dashes.
fn literal_spells_encoding(token: &proc_macro2::TokenTree) -> bool {
    let proc_macro2::TokenTree::Literal(literal) = token else {
        return false;
    };
    let text = literal.to_string();
    let content = text.trim_matches('"');
    content.len() > 4 && content.starts_with("--") && content.ends_with("--")
}

/// Whether an expression is a call of the owner.
fn is_call(expression: &Expr) -> bool {
    matches!(expression, Expr::Call(call) if names_encoder(&call.func))
}

/// Whether a callee path ends in the owner's name.
fn names_encoder(callee: &Expr) -> bool {
    let Expr::Path(path) = callee else {
        return false;
    };
    path.path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == ENCODER)
}

/// The line of the first local directory-name copy, if the file holds one.
fn copy(contents: &str) -> Option<usize> {
    for (index, line) in contents.lines().enumerate() {
        let replaces = line.contains(".replace(") || line.contains(".replacen(");
        let separators = line.contains("'/'") || line.contains("\"/\"");
        let dashes = line.contains("'-'") || line.contains("\"-\"");
        if replaces && separators && dashes {
            return Some(index + 1);
        }
        if line.contains("format!(\"--") && line.contains("--\"") {
            return Some(index + 1);
        }
    }
    None
}

/// Require one definition of the published encoder in its own module.
fn definitions(contents: &str) -> Result<(), String> {
    let definitions = contents
        .lines()
        .filter(|line| line.contains(&format!("fn {ENCODER}(")))
        .count();
    if definitions == 1 {
        Ok(())
    } else {
        Err(format!(
            "{OWNER_CRATE}::{ENCODER} must have exactly one definition, found {definitions}"
        ))
    }
}

/// The crate-relative path of one source, spelled with `/`.
fn relative_path(path: &Path, directory: &Path) -> String {
    path.strip_prefix(directory)
        .unwrap_or(path)
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}
