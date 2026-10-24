#![cfg(test)]

use crate::support;

use maestro_test_conventions::check_workspace;
use support::{Workspace, source};

/// A workspace holding the application, the adapter and a chat frontend.
fn workspace() -> Workspace {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-app", "maestro-rpc", "maestro-chat"]);
    workspace
}

/// A protocol record or the shared framing declared outside its owner, with the diagnostic.
const FOREIGN_DECLARATIONS: &[(&str, &str, &str, &str)] = &[
    (
        "maestro-chat",
        "src/records.rs",
        "//! Records.\n/// Record.\npub struct RpcResponse;\n",
        "RpcResponse declaration belongs to maestro-app or maestro-protocol",
    ),
    (
        "maestro-app",
        "src/rpc_mode/jsonl.rs",
        "//! Framing.\n/// Item.\npub fn serialize_json_line() {}\n",
        "serialize_json_line declaration belongs to maestro-json",
    ),
];

#[test]
fn rpc_records_and_framing_keep_their_owners() {
    assert_eq!(check_workspace(workspace().root.path()), Ok(()));
    for (member, file, contents, expected) in FOREIGN_DECLARATIONS {
        let workspace = workspace();
        source(&workspace, member, file, contents);
        let error = check_workspace(workspace.root.path()).expect_err(file);
        assert!(
            error.contains(expected),
            "{file}: expected {expected:?} in {error:?}"
        );
    }
}

/// Application sources that poll with a no-op waker outside the owner: a plain call, a
/// macro argument, a closure and an impl method.
const INLINE_POLLS: &[(&str, &str)] = &[
    (
        "src/plain.rs",
        "//! Plain.\n/// Item.\npub fn start() {\n    let _ = std::task::Waker::noop();\n}\n",
    ),
    (
        "src/macro.rs",
        "//! Macro.\n/// Item.\npub fn start() -> usize {\n    vec![Waker::noop()].len()\n}\n",
    ),
    (
        "src/closure.rs",
        "//! Closure.\n/// Item.\npub fn start() -> impl Fn() {\n    || drop(Waker::noop())\n}\n",
    ),
    (
        "src/method.rs",
        "//! Method.\n/// Item.\npub struct Task;\nimpl Task {\n    /// Starts.\n    pub fn start(&self) {\n        drop(Context::from_waker(Waker::noop()));\n    }\n}\n",
    ),
];

#[test]
fn inline_polls_delegate_to_start_now() {
    let owner = "//! Tasks.\n/// Starts.\npub fn start_now() {\n    drop(Context::from_waker(Waker::noop()));\n}\n";
    let control = workspace();
    source(&control, "maestro-app", "src/utils/task.rs", owner);
    source(&control, "maestro-app", "src/tests.rs", owner);
    assert_eq!(check_workspace(control.root.path()), Ok(()));
    for (file, contents) in INLINE_POLLS {
        let workspace = workspace();
        source(&workspace, "maestro-app", file, contents);
        let error = check_workspace(workspace.root.path()).expect_err(file);
        assert!(
            error.contains("inline poll outside utils::task::start_now"),
            "{file}: {error}"
        );
    }
}
