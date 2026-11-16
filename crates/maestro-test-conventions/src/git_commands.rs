//! Native Git test commands belong to the isolated test-process owner.

use crate::source::Member;
use proc_macro2::{TokenStream, TokenTree};

/// What a test constructor must use instead of the Git executable.
const OWNER: &str = "maestro_test_process::git_command::command";

/// Reject test constructors that bypass the isolated Git command owner.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let mut diagnostics: Vec<_> = members
        .iter()
        .flat_map(|member| {
            member.sources.iter().flat_map(|source| {
                source
                    .walk
                    .iter()
                    .flat_map(|walk| &walk.git_commands)
                    .map(|line| {
                        format!(
                            "{}:{line}: test Git commands belong to {OWNER}",
                            source.path.display()
                        )
                    })
            })
        })
        .collect();
    diagnostics.sort();
    diagnostics.dedup();
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics.join("\n"))
    }
}

/// Whether a literal names a Git executable, including an authored absolute path.
fn git(program: &str) -> bool {
    matches!(program.rsplit(['/', '\\']).next(), Some("git" | "git.exe"))
}

/// The line of a direct Git constructor, including a renamed command type.
pub(crate) fn call(call: &syn::ExprCall) -> Option<usize> {
    let syn::Expr::Path(path) = call.func.as_ref() else {
        return None;
    };
    if path
        .path
        .segments
        .last()
        .is_none_or(|segment| segment.ident != "new")
    {
        return None;
    }
    let syn::Expr::Lit(expression) = call.args.first()? else {
        return None;
    };
    let syn::Lit::Str(program) = &expression.lit else {
        return None;
    };
    git(&program.value()).then_some(program.span().start().line)
}

/// Direct Git constructors spelled inside macro arguments, without expanding macros.
pub(crate) fn tokens(stream: TokenStream) -> Vec<usize> {
    let mut lines = Vec::new();
    let mut tokens = stream.into_iter().peekable();
    while let Some(token) = tokens.next() {
        if let TokenTree::Ident(name) = &token
            && name == "new"
            && let Some(TokenTree::Group(arguments)) = tokens.peek()
            && let Some(TokenTree::Literal(program)) = arguments.stream().into_iter().next()
            && let Ok(program) = syn::parse_str::<syn::LitStr>(&program.to_string())
            && git(&program.value())
        {
            lines.push(name.span().start().line);
        }
        if let TokenTree::Group(group) = token {
            lines.extend(self::tokens(group.stream()));
        }
    }
    lines
}

/// Git-constructor ownership boundaries.
#[cfg(test)]
mod tests {
    use super::*;

    /// Test layouts and explicit gates constrain Git constructors, not the crate's name.
    #[test]
    fn git_guards_follow_test_layouts_and_gates() {
        let root = std::path::Path::new("/owner");
        let production = "fn command() { let _ = std::process::Command::new(\"git\"); }";
        for (file, text, allowed) in [
            ("src/commands.rs", production.to_owned(), true),
            ("tests/support.rs", production.to_owned(), false),
            ("src/tests/support.rs", production.to_owned(), false),
            (
                "src/inline.rs",
                format!("#[cfg(test)] mod tests {{ {production} }}"),
                false,
            ),
            ("src/gated.rs", format!("#[cfg(test)] {production}"), false),
            (
                "src/gated_file.rs",
                format!("#![cfg(test)]\n{production}"),
                false,
            ),
        ] {
            let member =
                Member::with_sources("maestro-test-conventions", root, [(root.join(file), text)]);
            assert_eq!(check(&[member]).is_ok(), allowed, "{file}");
        }
    }
}
