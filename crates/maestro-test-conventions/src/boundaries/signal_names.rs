//! Single-owner rule for the native signal names.

use std::str::FromStr;

use proc_macro2::{TokenStream, TokenTree};

use crate::source::{Member, Source, test_scope};

/// Crate that names the signals a child process can end with.
const OWNER: &str = "maestro-process";

/// Relative path of the file that names them.
const OWNER_FILE: &str = "src/child_process/exit_signal.rs";

/// Shortest signal suffix after the `SIG` prefix.
const SHORTEST_SUFFIX: usize = 3;

/// Reject a signal-name literal outside its owner.
///
/// A caller that formats its own signal name needs the whole table back, so the
/// fragment is the literal itself: a `SIG`-prefixed name anywhere in production
/// syntax belongs to the process owner. The walk reads the token stream of the
/// file, which is why a copy inside a macro argument, a closure or an impl method
/// is reported like a bare one. Test code stays free to write a signal name into
/// the script it launches.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let mut diagnostics = Vec::new();
    for member in members {
        for source in &member.sources {
            if test_scope(member, source) || owns_signals(member, source) {
                continue;
            }
            let Ok(tokens) = TokenStream::from_str(&source.contents) else {
                continue;
            };
            let file = source.path.display().to_string();
            let mut found = Vec::new();
            signal_literals(tokens, &mut found);
            diagnostics.extend(found.into_iter().map(|line| (file.clone(), line)));
        }
    }
    if diagnostics.is_empty() {
        return Ok(());
    }
    diagnostics.sort();
    diagnostics.dedup();
    Err(diagnostics
        .into_iter()
        .map(|(file, line)| {
            format!(
                "{file}:{line}: signal name belongs to {OWNER}::{OWNER_FILE}; use \
                 maestro_process::exit_signal_name"
            )
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

/// Whether `source` is the file that names the signals.
fn owns_signals(member: &Member, source: &Source) -> bool {
    member.name == OWNER
        && source
            .path
            .strip_prefix(&member.directory)
            .is_ok_and(|relative| relative == std::path::Path::new(OWNER_FILE))
}

/// Collect the line of every signal-name literal, descending into every group.
fn signal_literals(tokens: TokenStream, found: &mut Vec<usize>) {
    for token in tokens {
        match token {
            TokenTree::Literal(literal) if is_signal_name(&literal.to_string()) => {
                found.push(literal.span().start().line);
            }
            TokenTree::Group(group) => signal_literals(group.stream(), found),
            _ => {}
        }
    }
}

/// Whether a literal spells one signal name.
fn is_signal_name(literal: &str) -> bool {
    let Some(name) = literal
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    else {
        return false;
    };
    name.strip_prefix("SIG").is_some_and(|suffix| {
        suffix.len() >= SHORTEST_SUFFIX
            && suffix
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
    })
}
