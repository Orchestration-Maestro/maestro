#![cfg(test)]

use crate::support;

use maestro_test_conventions::check_workspace;
use support::{Workspace, source};

/// A workspace with the guest crate that ships the example tests.
fn workspace() -> Workspace {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-extensions-wasm"]);
    workspace
}

/// One example test that must be rejected: file name, contents and diagnostic.
type Bypass = (&'static str, &'static str, &'static str);

/// The shared example operations written inline by a per-example test.
const BYPASSES: &[Bypass] = &[
    (
        "walk_example.rs",
        "//! Walk.\n/// Releases every registration the host queued.\nfn release_in_order(host: &Record) {\n    loop {\n        let next = host.observe().next_to_release();\n        let Some((_, key)) = next else { break };\n        release(key);\n    }\n}\n",
        "example_support::release_all",
    ),
    (
        "macro_walk_example.rs",
        "//! Macro.\n/// Releases every registration inside a macro argument.\nfn released(host: &Record) {\n    assert!(host.observe().next_to_release().is_some());\n}\n",
        "example_support::release_all",
    ),
    (
        "closure_walk_example.rs",
        "//! Closure.\n/// Releases every registration from a closure.\nfn released(host: &Record) {\n    let drain = || host.observe().next_to_release();\n    drain();\n}\n",
        "example_support::release_all",
    ),
];

#[test]
fn example_helpers_reject_inline_copies() {
    let workspace = workspace();
    let mut accepted = Vec::new();
    for (name, contents, expected) in BYPASSES {
        let path = source(
            &workspace,
            "maestro-extensions-wasm",
            &format!("src/tests/{name}"),
            contents,
        );
        match check_workspace(workspace.root.path()) {
            Ok(()) => accepted.push(*name),
            Err(error) => {
                assert!(
                    error.contains(expected),
                    "{name}: expected {expected:?} in {error:?}"
                );
                assert!(
                    error.contains(&path.display().to_string()),
                    "{name}: {error}"
                );
            }
        }
        std::fs::remove_file(path).unwrap();
        assert_eq!(check_workspace(workspace.root.path()), Ok(()), "{name}");
    }
    assert!(accepted.is_empty(), "accepted bypasses: {accepted:?}");
}

#[test]
fn example_helpers_accept_the_shared_calls() {
    let workspace = workspace();
    source(
        &workspace,
        "maestro-extensions-wasm",
        "src/tests/example_support.rs",
        "//! Support.\n/// Parses the result.\npub fn result_value(text: &str) -> Value {\n    serde_json::from_str(text).unwrap()\n}\n",
    );
    source(
        &workspace,
        "maestro-extensions-wasm",
        "src/tests/hello_example.rs",
        "//! Hello.\n/// Releases the single tool the example registered.\nfn greet(host: &Record) {\n    release_all(host, &[\"tool hello\"]);\n    let _ = result_value(encoded);\n    let _ = serde_json::from_str(include_str!(\"hello_expected.json\"));\n    let _ = serde_json::from_str::<Value>(document);\n    let _ = serde_json::from_str::<Map<String, Value>>(&body[\"document\"]);\n    let _ = row[\"duration\"].as_str();\n}\n",
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}
