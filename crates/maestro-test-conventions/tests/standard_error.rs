//! Standard error diagnostics go through the shared writer, through the public workspace check.
#![cfg(test)]

use crate::support;

use maestro_test_conventions::check_workspace;
use support::{Workspace, source};

/// Write one member's source and compare the complete located diagnostics.
fn check(member: &str, contents: &str, expected: &[usize]) {
    let workspace = Workspace::new();
    workspace.foundation(&[member]);
    let path = source(&workspace, member, "src/lib.rs", contents);
    let result = if expected.is_empty() {
        Ok(())
    } else {
        Err(expected
            .iter()
            .map(|line| {
                format!(
                    "{}:{line}: production code prints to standard error with a print macro, which panics when the stream is closed; call maestro_agent::diagnostics::write_stderr or write_stderr_line",
                    path.display()
                )
            })
            .collect::<Vec<_>>()
            .join("\n"))
    };
    assert_eq!(check_workspace(workspace.root.path()), result);
}

#[test]
fn production_print_macros_to_standard_error_are_rejected() {
    check(
        "maestro-tools",
        r#"//! Tools.
/// Plain statement.
pub fn statement() {
    eprintln!("lost");
}

/// Without a newline, through a path.
pub fn qualified() {
    std::eprint!("lost");
}

/// Inside a closure.
pub fn closure() -> impl Fn() {
    || eprintln!("lost")
}

/// Inside an associated function.
pub struct Holder;
impl Holder {
    /// Method.
    pub fn report(&self) {
        eprintln!("lost");
    }
}

/// Inside another macro's arguments.
pub fn nested() {
    drop(vec![eprint!("lost")]);
}
"#,
        &[4, 9, 14, 22, 28],
    );
}

#[test]
fn test_code_tooling_and_fallible_writes_are_accepted() {
    check(
        "maestro-tools",
        r#"//! Tools.
use std::io::Write;

/// Fallible write that drops the failure.
pub fn fallible() {
    drop(writeln!(std::io::stderr(), "kept"));
}

/// A gated statement is test code.
pub fn traced() {
    #[cfg(test)]
    eprintln!("trace");
}

#[cfg(test)]
mod tests {
    #[test]
    fn expectation() {
        eprintln!("premise not met");
    }
}
"#,
        &[],
    );
    check(
        "maestro-tooling",
        "//! Tooling.\n\n/// Report.\npub fn report() {\n    eprintln!(\"developer terminal\");\n}\n",
        &[],
    );
}
