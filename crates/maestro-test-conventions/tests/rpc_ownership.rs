#![cfg(test)]

use crate::support;

use maestro_test_conventions::check_workspace;
use support::{Workspace, source};

/// The application's record writer, in the one delegating shape.
const WRITER: &str = "//! Server.\n/// Writes one record.\nfn write_record(output: &RpcOutput, record: &impl Serialize) -> io::Result<()> {\n    let line = serialize_json_line(&serde_json::to_value(record)?)?;\n    output(&line)\n}\n";

/// A workspace holding the application, the adapter and a chat frontend, with the writer.
fn workspace() -> Workspace {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-app", "maestro-rpc", "maestro-chat"]);
    source(&workspace, "maestro-app", "src/rpc_mode/mod.rs", WRITER);
    workspace
}

/// One source that must be rejected: member, file, contents and its diagnostic.
type Bypass = (&'static str, &'static str, &'static str, &'static str);

/// Second framers, second envelopes, discarded or rewritten writes and foreign records.
const BYPASSES: &[Bypass] = &[
    (
        "maestro-app",
        "src/rpc_mode/framer.rs",
        "//! Framer.\n/// Item.\npub fn frame(text: &str) -> String {\n    format!(\"{text}\\n\")\n}\n",
        "hand-built line ending",
    ),
    (
        "maestro-rpc",
        "src/server/rpc_mode/native.rs",
        "//! Header.\n/// Item.\npub fn header() -> serde_json::Value {\n    serde_json::json!({\"type\": \"response\"})\n}\n",
        "hand-built response envelope",
    ),
    (
        "maestro-app",
        "src/rpc_mode/discard.rs",
        "//! Discard.\n/// Item.\npub fn write(value: &serde_json::Value) {\n    let _ = maestro_json::jsonl::serialize_json_line(value);\n}\n",
        "shared framing called outside the record writer",
    ),
    (
        "maestro-app",
        "src/rpc_types/macro.rs",
        "//! Macro.\n/// Item.\npub fn lines(value: &serde_json::Value) -> usize {\n    vec![serialize_json_line(value)].len()\n}\n",
        "shared framing called outside the record writer",
    ),
    (
        "maestro-app",
        "src/rpc_mode/method.rs",
        "//! Method.\n/// Item.\npub struct Writer {\n    /// Output.\n    output: RpcOutput,\n}\nimpl Writer {\n    /// Writes.\n    pub fn write(&self, line: &str) {\n        drop((self.output)(line));\n    }\n}\n",
        "output written outside the record writer",
    ),
    (
        "maestro-app",
        "src/rpc_mode/closure.rs",
        "//! Closure.\n/// Item.\npub fn writer(output: RpcOutput) -> impl Fn(&str) {\n    move |line| drop(output(line))\n}\n",
        "output written outside the record writer",
    ),
    (
        "maestro-app",
        "src/rpc_mode/mod.rs",
        "//! Server.\n/// Writes one record.\nfn write_record(output: &RpcOutput, record: &impl Serialize) -> io::Result<()> {\n    let _ = serialize_json_line(&serde_json::to_value(record)?)?;\n    output(\"{}\")\n}\n",
        "the record writer must write the shared line whole",
    ),
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
fn rpc_writers_delegate_to_shared_owners() {
    assert_eq!(check_workspace(workspace().root.path()), Ok(()));
    for (member, file, contents, expected) in BYPASSES {
        let workspace = workspace();
        source(&workspace, member, file, contents);
        let error = check_workspace(workspace.root.path()).expect_err(file);
        assert!(
            error.contains(expected),
            "{file}: expected {expected:?} in {error:?}"
        );
    }
    let workspace = workspace();
    let accepted = "//! Adapter.\n/// Item.\npub fn serve(output: RpcOutput, input: u8) {\n    run(input, output, \"text\");\n}\n";
    source(
        &workspace,
        "maestro-rpc",
        "src/server/rpc_mode/mod.rs",
        accepted,
    );
    let tests = "//! Tests.\n/// Helper.\npub fn helper(output: RpcOutput) {\n    drop(output(\"{}\\n\"));\n}\n";
    source(&workspace, "maestro-app", "src/rpc_mode/tests.rs", tests);
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
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
