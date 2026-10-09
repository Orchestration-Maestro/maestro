//! Native attachment trees, consumer fixtures and controlled children.
#![cfg(test)]
use super::{attachments, completion_native::Tree};
use maestro_cancellation::Cancellation;
use maestro_tui::autocomplete::{CompletionOptions, CursorPosition, NativeAutocompleteOperations};
use maestro_tui::{AutocompleteProvider, AutocompleteSuggestions, CombinedAutocompleteProvider};
/// Installed executable is a required integration operand.
pub(crate) fn executable() -> String {
    let output = std::process::Command::new("fd")
        .arg("--version")
        .output()
        .expect("fd installed");
    assert!(output.status.success());
    "fd".into()
}
/// Query through the same native provider used by callers.
pub(crate) async fn query(base: &str, text: &str) -> Option<AutocompleteSuggestions> {
    let provider = CombinedAutocompleteProvider::new(
        vec![],
        base.into(),
        Some(executable()),
        NativeAutocompleteOperations::default(),
    );
    provider
        .get_suggestions(
            &[text.into()],
            CursorPosition {
                line: 0,
                col: text.len(),
            },
            CompletionOptions {
                signal: &Cancellation::new(),
                force: None,
            },
        )
        .await
        .unwrap()
}
/// Read ordered paths while checking all candidate fields.
pub(crate) fn paths(result: Option<AutocompleteSuggestions>, prefix: &str) -> Vec<String> {
    let result = result.unwrap();
    assert_eq!(result.prefix, prefix);
    result
        .items
        .iter()
        .map(|item| {
            let path = item.description.as_ref().unwrap();
            let suffix = if item.label.ends_with('/') { "/" } else { "" };
            assert_eq!(
                item.label,
                format!("{}{suffix}", maestro_path::basename(path, None))
            );
            let completion = format!("{path}{suffix}");
            assert_eq!(
                item.value,
                if prefix.starts_with("@\"") || completion.contains(' ') {
                    format!("@\"{completion}\"")
                } else {
                    format!("@{completion}")
                }
            );
            completion
        })
        .collect()
}
/// Create nested named fixture entries.
pub(crate) fn entries(tree: &Tree, names: &[&str]) {
    for name in names {
        let path = tree.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"").unwrap();
    }
}

/// A real search consumer query with all returned fields retained.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeCase {
    /// Diagnostic identity.
    id: String,
    /// Owning test subset.
    group: String,
    /// Input token.
    text: String,
    /// Files created for this query.
    names: Vec<String>,
    /// Complete suggestion output.
    result: Option<attachments::Suggestions>,
    /// Complete insertion output for every selected candidate.
    applications: Vec<attachments::Application>,
}
/// Compare real process results against recorded consumer observations.
pub(crate) async fn native_cases(group: &str) {
    let cases: Vec<NativeCase> =
        serde_json::from_str(include_str!("native_attachments.json")).unwrap();
    for case in cases.iter().filter(|c| c.group == group) {
        let tree = Tree::new().unwrap();
        entries(
            &tree,
            &case.names.iter().map(String::as_str).collect::<Vec<_>>(),
        );
        if case.id == "hidden_git" {
            std::fs::write(tree.0.join(".gitignore"), "").unwrap();
        }
        if case.id == "ignored" {
            std::fs::write(tree.0.join(".gitignore"), "ignored.txt\n").unwrap();
        }
        let provider = CombinedAutocompleteProvider::new(
            vec![],
            tree.authored(),
            Some(executable()),
            NativeAutocompleteOperations::default(),
        );
        let lines = [case.text.clone()];
        let cursor = CursorPosition {
            line: 0,
            col: case.text.len(),
        };
        let result = provider
            .get_suggestions(
                &lines,
                cursor,
                CompletionOptions {
                    signal: &Cancellation::new(),
                    force: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(
            result,
            case.result.as_ref().map(attachments::Suggestions::public),
            "{}",
            case.id
        );
        if let Some(result) = result {
            assert_eq!(result.items.len(), case.applications.len());
            for (item, expected) in result.items.iter().zip(&case.applications) {
                let applied = provider.apply_completion(&lines, cursor, item, &result.prefix);
                assert_eq!(applied.lines, expected.lines, "{}", case.id);
                assert_eq!(
                    (applied.cursor_line, applied.cursor_col),
                    (expected.cursor_line, expected.cursor_col),
                    "{}",
                    case.id
                );
            }
        }
    }
}

/// An owned child script, invoked directly without adapter shell interpolation.
#[cfg(unix)]
pub(crate) fn child_script(tree: &Tree, source: &str) -> String {
    use std::os::unix::fs::PermissionsExt;
    let path = tree.0.join("child");
    std::fs::write(&path, format!("#!/usr/bin/env python3\n{source}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    path.to_str().unwrap().into()
}

/// Caller-owned runtime for native child operations.
pub(crate) fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}
