//! Native attachment trees, consumer fixtures and controlled children.
#![cfg(test)]
#[cfg(unix)]
use super::attachments;
use super::completion_native::Tree;
use maestro_cancellation::Cancellation;
use maestro_tui::autocomplete::{CompletionOptions, CursorPosition, NativeAutocompleteOperations};
use maestro_tui::{AutocompleteProvider, AutocompleteSuggestions, CombinedAutocompleteProvider};
use std::future::Future;
/// Run real-search assertions with absent or unusable ambient search tools.
///
/// Each child receives a copied pinned program separately from PATH, so host tools cannot
/// satisfy a missing fixture or change completion results. Only the child runs the body.
pub(crate) fn isolated(body: impl Future<Output = ()>) {
    if maestro_test_process::selection().is_child() {
        runtime().block_on(body);
        return;
    }
    let bin = Tree::new().unwrap();
    let fd = maestro_test_process::install_search_program("fd", bin.path()).unwrap();
    let ambient = Tree::new().unwrap();
    for name in ["fd", "rg"] {
        maestro_test_process::install_command(
            &ambient
                .path()
                .join(format!("{name}{}", std::env::consts::EXE_SUFFIX)),
            r#"{"scenario":"output","stdout":[],"stderr":[],"code":99}"#,
        )
        .unwrap();
    }
    for path in [std::ffi::OsStr::new(""), ambient.path().as_os_str()] {
        let output =
            maestro_test_process::TestProcess::current(std::thread::current().name().unwrap())
                .unwrap()
                .case("search-path")
                .nocapture()
                .env("PATH", path)
                .env("MAESTRO_TEST_FD", &fd)
                .output()
                .unwrap();
        assert!(
            output.status.success(),
            "PATH={path:?}\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// The isolated child's pinned executable, independent of its search PATH.
pub(crate) fn executable() -> String {
    std::env::var("MAESTRO_TEST_FD").expect("isolated child supplies the pinned executable")
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
        let path = tree.path().join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"").unwrap();
    }
}

/// A real search consumer query with all returned fields retained.
#[cfg(unix)]
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
    #[serde(default)]
    applications: Vec<attachments::Application>,
}
/// Compare real process results against recorded consumer observations.
#[cfg(unix)]
pub(crate) async fn native_cases(group: &str) {
    let cases: Vec<NativeCase> =
        serde_json::from_str(include_str!("native_attachments.json")).unwrap();
    for case in cases.iter().filter(|c| c.group == group) {
        let tree = Tree::new().unwrap();
        let names: Vec<_> = case.names.iter().map(String::as_str).collect();
        if !tree.keeps_apart(&case.id, &names) {
            continue;
        }
        entries(&tree, &names);
        if case.id == "hidden_git" {
            std::fs::write(tree.path().join(".gitignore"), "").unwrap();
        }
        if case.id == "ignored" {
            std::fs::write(tree.path().join(".gitignore"), "ignored.txt\n").unwrap();
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

/// Install the prepared native fixture as one controlled completion child.
///
/// The sibling record selects a finite search-family scenario, so the executable
/// receives no adapter shell interpolation and no interpreted program.
#[cfg(unix)]
pub(crate) fn child(tree: &Tree, name: &str, record: &serde_json::Value) -> String {
    let artifact = std::env::var_os("MAESTRO_NATIVE_FIXTURE")
        .expect("nextest native-fixture setup supplies the artifact");
    let path = tree.path().join(name);
    let mut sibling = path.as_os_str().to_owned();
    sibling.push(".fixture");
    std::fs::write(sibling, format!("search\n{record}")).unwrap();
    maestro_test_process::install_executable(std::path::Path::new(&artifact), &path).unwrap();
    path.to_str().unwrap().into()
}

/// Caller-owned runtime for native child operations.
pub(crate) fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}
