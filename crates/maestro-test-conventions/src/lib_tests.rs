//! Library test-target agreement with the sources that a crate holds.

use ra_ap_rustc_lexer::TokenKind;

use crate::source::{Member, Source, read_manifest, test_layout};

/// Check that each library's `[lib] test` setting agrees with its in-source tests.
///
/// A library target that holds in-source tests and sets `test = false` would never run
/// them. A library target with no in-source tests and no flag still builds an empty test
/// executable on every test build.
pub(super) fn check(members: &[Member]) -> Result<(), String> {
    let mut diagnostics = Vec::new();
    for member in members.iter().filter(|member| library(member)) {
        if let Some(diagnostic) = diagnostic(member)? {
            diagnostics.push(diagnostic);
        }
    }
    if diagnostics.is_empty() {
        return Ok(());
    }
    Err(diagnostics.join("\n"))
}

/// Whether the member's sources hold a library root under `src`.
fn library(member: &Member) -> bool {
    member
        .sources
        .iter()
        .any(|source| source.path == member.directory.join("src").join("lib.rs"))
}

/// Report the disagreement between one library's manifest and its sources.
fn diagnostic(member: &Member) -> Result<Option<String>, String> {
    let manifest_path = member.directory.join("Cargo.toml");
    let manifest = read_manifest(&manifest_path)?;
    match (runnable(&manifest), holds_tests(member)) {
        (false, true) => Ok(Some(format!(
            "{}: in-source tests need a runnable library test target; drop `test = false` from [lib]",
            manifest_path.display()
        ))),
        (true, false) => Ok(Some(format!(
            "{}: a library with no in-source tests must set `test = false` in [lib]",
            manifest_path.display()
        ))),
        _ => Ok(None),
    }
}

/// Whether Cargo builds the library's own test executable.
fn runnable(manifest: &toml::Table) -> bool {
    manifest
        .get("lib")
        .and_then(|lib| lib.get("test"))
        .and_then(toml::Value::as_bool)
        != Some(false)
}

/// Whether the member's `src` holds in-source tests.
fn holds_tests(member: &Member) -> bool {
    member
        .sources
        .iter()
        .filter(|source| source.path.starts_with(member.directory.join("src")))
        .any(|source| test_source(member, source))
}

/// Whether the source holds test code: a test layout, a test gate or a test attribute.
fn test_source(member: &Member, source: &Source) -> bool {
    test_layout(&source.path, &member.directory)
        || source
            .walk
            .as_ref()
            .is_some_and(|walk| walk.gated || !walk.test_spans.is_empty())
        || test_attribute(source)
}

/// Whether a source declares a test attribute, such as `#[test]` or `#[tokio::test]`.
///
/// The lexer keeps comments and strings apart, so prose that mentions an attribute is not a
/// declaration. The attribute's last name decides, because a test macro keeps its own path.
fn test_attribute(source: &Source) -> bool {
    source.tokens.iter().enumerate().any(|(index, (kind, _))| {
        *kind == TokenKind::Pound
            && matches!(
                source.tokens.get(index + 1),
                Some((TokenKind::OpenBracket, _))
            )
            && attribute_name(source, index + 2) == Some("test")
    })
}

/// The last path segment of the attribute that starts at `from`.
///
/// The segment stops at the attribute's argument list or value, so a `cfg` predicate that
/// names the test configuration is not an attribute named `test`.
fn attribute_name(source: &Source, from: usize) -> Option<&str> {
    source.tokens[from..]
        .iter()
        .take_while(|(kind, _)| {
            !matches!(
                kind,
                TokenKind::OpenParen | TokenKind::Eq | TokenKind::CloseBracket
            )
        })
        .filter(|(kind, _)| matches!(kind, TokenKind::Ident))
        .last()
        .map(|(_, range)| &source.contents[range.clone()])
}
