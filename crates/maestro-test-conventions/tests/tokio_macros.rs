//! A member whose production code uses a Tokio macro declares Tokio's `macros` feature, through
//! the public workspace check.
#![cfg(test)]

use crate::support;

use maestro_test_conventions::check_workspace;
use support::{Workspace, source};

/// Production spellings of the macros that need the feature, after a test-gated item.
const USES: &str = r"//! Text.
#[cfg(test)]
thread_local! {
    static CLOCK: u8 = const { 0 };
}
use tokio::try_join;

/// Qualified invocation.
pub async fn waits() {
    tokio::select! {}
}

/// Entry attribute.
#[tokio::main]
pub async fn entry() {}

/// Invocation inside another macro's arguments.
pub fn nested() {
    drop(vec![tokio::join!()]);
}
";

/// Spellings that need no feature, and test code.
const EXEMPT: &str = r"//! Text.
/// Pinning is not gated.
pub fn pinned() {
    tokio::pin!(value);
}

/// Another crate's macro of the same name.
pub fn other() {
    futures::join!();
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn runs() {
        tokio::select! {}
    }
}
";

/// Write the member with its Tokio declaration and source, then run the workspace check.
fn check(features: &str, contents: &str) -> (Result<(), String>, std::path::PathBuf) {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-text"]);
    workspace.external("tokio");
    std::fs::write(
        workspace.root.path().join("external/Cargo.toml"),
        "[workspace]\n[package]\nname = \"tokio\"\nversion = \"9.0.0\"\nedition = \"2024\"\n[features]\nmacros = []\n",
    )
    .unwrap();
    workspace.member(
        "maestro-text",
        "maestro-text",
        &format!(
            "[dependencies]\ntokio = {{ path = \"../../external\", features = [{features}] }}"
        ),
    );
    let path = source(&workspace, "maestro-text", "src/lib.rs", contents);
    (check_workspace(workspace.root.path()), path)
}

#[test]
fn production_tokio_macros_without_the_feature_are_rejected() {
    let (result, path) = check("", USES);
    let expected = [6, 10, 14, 19]
        .map(|line| {
            format!(
                "{}:{line}: production code uses a tokio macro, but maestro-text does not declare tokio's `macros` feature",
                path.display()
            )
        })
        .join("\n");
    assert_eq!(result, Err(expected));
}

#[test]
fn declared_feature_and_exempt_spellings_are_accepted() {
    assert_eq!(check("\"macros\"", USES).0, Ok(()));
    assert_eq!(check("", EXEMPT).0, Ok(()));
}
