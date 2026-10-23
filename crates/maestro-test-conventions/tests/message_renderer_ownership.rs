#![cfg(test)]

use crate::support;

use maestro_test_conventions::check_workspace;
use support::Workspace;

/// Writes one chat production source and returns its path.
fn chat_source(workspace: &Workspace, name: &str, contents: &str) -> std::path::PathBuf {
    let path = workspace
        .root
        .path()
        .join("crates/maestro-chat/src")
        .join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&path, contents).unwrap();
    path
}

/// A chat workspace with the owner crate and its required edges.
fn workspace() -> Workspace {
    let workspace = Workspace::new();
    workspace.foundation(&["maestro-chat", "maestro-extensions"]);
    workspace
}

/// One chat source that must be rejected: file name, contents and the diagnostic it carries.
type Bypass = (&'static str, &'static str, &'static str);

/// Chat sources naming raw renderer plumbing in one import form each.
const BYPASSES: &[Bypass] = &[
    (
        "raw.rs",
        "//! Raw.\nuse maestro_app::MessageRenderer;\n/// Item.\npub fn render() -> Option<MessageRenderer> {\n    None\n}\n",
        "raw MessageRenderer outside maestro-extensions",
    ),
    (
        "alias.rs",
        "//! Alias.\nuse maestro_extensions::EventBus as Bus;\n/// Item.\npub fn bus() -> Option<Bus<()>> {\n    None\n}\n",
        "raw EventBus outside maestro-extensions",
    ),
    (
        "qualified.rs",
        "//! Qualified.\n/// Item.\npub fn call(emitter: u8) {\n    let _ = emitter.invoke_callback();\n}\n",
        "raw invoke_callback outside maestro-extensions",
    ),
    (
        "lookup.rs",
        "//! Lookup.\n/// Item.\npub fn lookup(runner: u8) {\n    let _ = maestro_app::ExtensionRunner::get_message_renderer::<u8>(&runner, \"note\");\n}\n",
        "raw get_message_renderer outside maestro-extensions",
    ),
    (
        "emitter.rs",
        "//! Emitter.\n/// Item.\npub struct Item<'a>(&'a CallbackEmitter<'a>);\n",
        "raw CallbackEmitter outside maestro-extensions",
    ),
    (
        "duplicate.rs",
        "//! Duplicate.\n/// Record.\npub struct MessageRendererCall;\n",
        "MessageRendererCall declaration belongs to maestro-extensions",
    ),
    (
        "imported.rs",
        "//! Imported.\nuse maestro_app::MessageRenderer as Raw;\n/// Item.\npub struct Holder;\n",
        "raw MessageRenderer outside maestro-extensions",
    ),
    (
        "segment.rs",
        "//! Segment.\n/// Item.\npub fn clone_of(value: &u8) -> u8 {\n    maestro_app::MessageRenderer::clone(value)\n}\n",
        "raw MessageRenderer outside maestro-extensions",
    ),
    (
        "grouped_self.rs",
        "//! Grouped self.\nuse maestro_app::MessageRenderer::{self as Raw};\n/// Item.\npub struct Holder;\n",
        "raw MessageRenderer outside maestro-extensions",
    ),
    (
        "deep_self.rs",
        "//! Deep self.\nuse maestro_app::{renderers::{MessageRenderer::{self as Raw}}};\n/// Item.\npub struct Holder;\n",
        "raw MessageRenderer outside maestro-extensions",
    ),
    (
        "nested_group.rs",
        "//! Nested group.\nuse maestro_app::{renderers::{CallbackEmitter}};\n/// Item.\npub struct Holder;\n",
        "raw CallbackEmitter outside maestro-extensions",
    ),
    (
        "grouped_rename.rs",
        "//! Grouped rename.\nuse maestro_app::{EventBus as Bus, ExtensionRunner};\n/// Item.\npub struct Holder;\n",
        "raw EventBus outside maestro-extensions",
    ),
    (
        "glob.rs",
        "//! Glob.\nuse maestro_app::get_message_renderer::*;\n/// Item.\npub struct Holder;\n",
        "raw get_message_renderer outside maestro-extensions",
    ),
    (
        "grouped_plain.rs",
        "//! Grouped plain.\nuse maestro_app::invoke_callback::{self};\n/// Item.\npub struct Holder;\n",
        "raw invoke_callback outside maestro-extensions",
    ),
];

#[test]
fn message_renderer_owner_rejects_raw_chat_bypasses() {
    let workspace = workspace();
    let mut accepted = Vec::new();
    for (name, contents, expected) in BYPASSES {
        let path = chat_source(&workspace, name, contents);
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

/// Writes one extensions production source and returns its path.
fn owner_source(workspace: &Workspace, name: &str, contents: &str) -> std::path::PathBuf {
    let path = workspace
        .root
        .path()
        .join("crates/maestro-extensions/src")
        .join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&path, contents).unwrap();
    path
}

#[test]
fn message_renderer_owner_requires_one_declaration_in_its_owner_file() {
    let workspace = workspace();
    let owner = workspace
        .root
        .path()
        .join("crates/maestro-extensions/src/types");
    std::fs::create_dir_all(&owner).unwrap();
    let declaration = owner.join("renderers.rs");
    std::fs::write(
        &declaration,
        "//! Renderers.\n/// Record.\npub struct MessageRendererCall<T = u8>(T);\n",
    )
    .unwrap();
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    let nested = owner_source(
        &workspace,
        "nested/types/renderers.rs",
        "//! Nested.\n/// Record.\npub struct MessageRendererCall;\n",
    );
    let error = check_workspace(workspace.root.path()).unwrap_err();
    assert!(
        error.contains("duplicate MessageRendererCall declaration"),
        "{error}"
    );
    assert!(error.contains(&nested.display().to_string()), "{error}");
    std::fs::remove_file(nested).unwrap();
    std::fs::write(
        &declaration,
        "//! Renderers.\n/// Record.\npub struct MessageRendererCall<T = u8>(T);\n/// Inner.\npub mod inner {\n    /// Record.\n    pub struct MessageRendererCall;\n}\n",
    )
    .unwrap();
    let error = check_workspace(workspace.root.path()).unwrap_err();
    assert!(
        error.contains("second MessageRendererCall declaration"),
        "{error}"
    );
}

#[test]
fn message_renderer_owner_accepts_projected_calls_and_test_scopes() {
    let workspace = workspace();
    let accepted: &[(&str, &str)] = &[
        (
            "projected.rs",
            "//! Projected.\nuse maestro_app::{MessageRendererCall, MessageRenderOptions};\n/// Item.\npub fn call(call: &MessageRendererCall) {\n    let _ = (call, MessageRenderOptions { expanded: false });\n}\n",
        ),
        (
            "lookup.rs",
            "//! Lookup.\n/// Item.\npub fn lookup(runner: &maestro_app::ExtensionRunner) {\n    let _ = runner.get_message_renderer_call::<u8>(\"note\");\n}\n",
        ),
        (
            "decoy.rs",
            "//! Decoy.\n/// Item.\npub const TEXT: &str = \"MessageRenderer EventBus invoke_callback\";\n// MessageRenderer in an ordinary comment.\n",
        ),
        (
            "nested_allowed.rs",
            "//! Nested allowed.\nuse maestro_app::{renderers::{MessageRendererCall, MessageRenderOptions}};\n/// Item.\npub struct Holder(MessageRendererCall, MessageRenderOptions);\n",
        ),
        (
            "tests.rs",
            "//! Test scope.\nuse maestro_app::MessageRenderer;\n/// Helper.\npub fn helper() -> Option<MessageRenderer> {\n    None\n}\n",
        ),
        (
            "gated.rs",
            "//! Gated.\n#[cfg(test)]\nmod tests {\n    use maestro_app::EventBus;\n    /// Helper.\n    pub fn helper(_bus: &EventBus) {}\n}\n",
        ),
    ];
    for (name, contents) in accepted {
        let path = chat_source(&workspace, name, contents);
        assert_eq!(
            check_workspace(workspace.root.path()),
            Ok(()),
            "{name}: {}",
            std::fs::read_to_string(&path).unwrap()
        );
        std::fs::remove_file(path).unwrap();
    }
    let owner = workspace
        .root
        .path()
        .join("crates/maestro-extensions/src/types");
    std::fs::create_dir_all(&owner).unwrap();
    let declaration = owner.join("renderers.rs");
    std::fs::write(
        &declaration,
        "//! Renderers.\n/// Record.\npub struct MessageRendererCall<T = u8>(T);\n",
    )
    .unwrap();
    assert_eq!(check_workspace(workspace.root.path()), Ok(()));
    let duplicate = owner.join("duplicate.rs");
    std::fs::write(
        &duplicate,
        "//! Duplicate.\n/// Record.\npub struct MessageRendererCall;\n",
    )
    .unwrap();
    let error = check_workspace(workspace.root.path()).unwrap_err();
    assert!(
        error.contains("duplicate MessageRendererCall declaration"),
        "{error}"
    );
}
