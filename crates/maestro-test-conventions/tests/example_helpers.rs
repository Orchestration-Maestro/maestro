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
        "decode_example.rs",
        "//! Decode.\n/// Reads the schema out of the registered metadata.\nfn tool_metadata(registered: &str) {\n    let mut metadata: serde_json::Value = serde_json::from_str(registered).unwrap();\n    metadata[\"parameters\"] =\n        serde_json::from_str(metadata[\"parameters\"].as_str().unwrap()).unwrap();\n}\n",
        "example_support::decoded",
    ),
    (
        "walk_example.rs",
        "//! Walk.\n/// Releases every registration the host queued.\nfn release_in_order(host: &Record) {\n    loop {\n        let next = host.observe().next_to_release();\n        let Some((_, key)) = next else { break };\n        release(key);\n    }\n}\n",
        "example_support::release_all",
    ),
    (
        "discard_example.rs",
        "//! Discard.\n/// Drains every registration without comparing them.\nfn drained(host: &Record) {\n    release_all(host);\n}\n",
        "compare the names example_support::release_all returns",
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
        "//! Support.\n/// Decodes the member.\npub fn decoded(text: &str, member: &str) -> Value {\n    let mut value: Value = serde_json::from_str(text).unwrap();\n    value[member] = serde_json::from_str(value[member].as_str().unwrap()).unwrap();\n    value\n}\n",
    );
    source(
        &workspace,
        "maestro-extensions-wasm",
        "src/tests/hello_example.rs",
        "//! Hello.\n/// Releases the single tool the example registered.\nfn greet(host: &Record) {\n    assert_eq!(release_all(host), [\"tool hello\"]);\n    let _ = decoded(registered, \"parameters\");\n    let _ = serde_json::from_str(include_str!(\"hello_expected.json\"));\n    let _ = row[\"duration\"].as_str();\n}\n",
    );
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
}
